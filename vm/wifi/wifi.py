#!/usr/bin/env python3
"""Fake Wi-Fi for the Nickel VM.

Runs natively in a sidecar container that shares the VM's network namespace
and its /run/dbus, /run/wpa_supplicant and /tmp. It stands in for the parts
of Nickel's Wi-Fi stack that need hardware:

- renames the container's eth0 to wlan0, so `ifconfig wlan0` finds it;
- owns name.marples.roy.dhcpcd on the system bus (the dhcpcd-dbus API Nickel
  uses for scanning, networks and the connection state), with one network,
  VM-WLAN, that is known and connects at once;
- answers wpa_supplicant's control socket /run/wpa_supplicant/wlan0.

The real wpa_supplicant and dhcpcd are replaced by vm/stub.sh. Nickel's
connectivity check goes to the internet through the container's network.
"""

import asyncio
import ipaddress
import os
import socket
import subprocess
import sys

from dbus_next import BusType, Variant
from dbus_next.aio import MessageBus
from dbus_next.service import ServiceInterface, method, signal

IFACE = 'wlan0'
SSID = 'VM-WLAN'
BSSID = '02:00:00:00:00:01'
FREQ = 2437
LEVEL = -42
WPA_DIR = '/run/wpa_supplicant'
READY = '/run/dbus/vm-wifi-ready'


def log(*args):
    print('vm-wifi:', *args, flush=True)


def trace(*args):
    if os.environ.get('VM_WIFI_TRACE'):
        log(*args)


def ip(*args):
    return subprocess.run(['ip', *args], capture_output=True, text=True)


NET = {}  # address and gateway docker gave eth0, restored on every connect


def rename_interface():
    """eth0 -> wlan0; remembers its address and default route."""
    out = ip('-4', '-o', 'addr', 'show', 'eth0').stdout.split()
    route = ip('-4', 'route', 'show', 'default').stdout.split()
    NET['addr'] = out[out.index('inet') + 1]
    NET['gw'] = route[route.index('via') + 1]
    ip('link', 'set', 'eth0', 'down')
    ip('link', 'set', 'eth0', 'name', IFACE)
    ip('link', 'set', IFACE, 'up')
    bring_up()
    log('eth0 is now', IFACE, NET['addr'], 'gateway', NET['gw'])


def bring_up():
    """What dhcpcd would do on BOUND: link, address, default route. Nickel
    takes wlan0 down when it switches Wi-Fi off, which drops the route."""
    ip('link', 'set', IFACE, 'up')
    ip('addr', 'replace', NET['addr'], 'dev', IFACE)
    ip('route', 'replace', 'default', 'via', NET['gw'], 'dev', IFACE)


def link_up():
    return 'state UP' in ip('link', 'show', IFACE).stdout or \
        ',UP' in ip('link', 'show', IFACE).stdout


def address():
    net = ipaddress.ip_interface(NET['addr'])
    return net, NET['gw']


def u32(addr):
    # dhcpcd-dbus sends addresses as uint32 in network byte order.
    return int.from_bytes(ipaddress.ip_address(addr).packed, 'little')


class State:
    connected = False
    networks = {0: SSID}  # id -> ssid, as in wpa_supplicant.conf


STATE = State()


class Dhcpcd(ServiceInterface):
    def __init__(self):
        super().__init__('name.marples.roy.dhcpcd')

    def iface_info(self):
        net, gw = address()
        info = {
            'Interface': Variant('s', IFACE),
            'Reason': Variant('s', 'BOUND' if STATE.connected else 'NOCARRIER'),
            'Up': Variant('b', True),
            'Wireless': Variant('b', True),
            'Flags': Variant('u', 0x1043),
            'Metric': Variant('i', 3000),
        }
        if STATE.connected:
            info['SSID'] = Variant('s', SSID)
            if net:
                info['IPAddress'] = Variant('u', u32(net.ip))
                info['SubnetCIDR'] = Variant('y', net.network.prefixlen)
                info['SubnetMask'] = Variant('u', u32(net.netmask))
            if gw:
                info['Routers'] = Variant('au', [u32(gw)])
                info['DomainNameServers'] = Variant('au', [u32(gw)])
        return info

    @method()
    def GetVersion(self) -> 's':
        trace('GetVersion')
        return '0.6.1'

    @method()
    def GetDhcpcdVersion(self) -> 's':
        trace('GetDhcpcdVersion')
        return '9.4.1'

    @method()
    def ListInterfaces(self) -> 'as':
        trace('ListInterfaces')
        return [IFACE]

    @method()
    def GetInterfaces(self) -> 'a{sa{sv}}':
        trace('GetInterfaces')
        return {IFACE: self.iface_info()}

    @method()
    def GetStatus(self) -> 's':
        forget_if_down()
        trace('GetStatus')
        return 'connected' if STATE.connected else 'disconnected'

    @method()
    def Rebind(self, interface: 's'):
        log('Rebind', interface)
        asyncio.get_event_loop().create_task(connect(self))

    @method()
    def Release(self, interface: 's'):
        log('Release', interface)
        disconnect(self)

    @method()
    def Stop(self, interface: 's'):
        log('Stop', interface)
        disconnect(self)

    @method()
    def GetConfigBlocks(self, block: 's') -> 'as':
        trace('GetConfigBlocks')
        return []

    @method()
    def GetConfig(self, block: 's', name: 's') -> 'a(ss)':
        trace('GetConfig')
        return []

    @method()
    def SetConfig(self, block: 's', name: 's', config: 'a(ss)'):
        pass

    @method()
    def Scan(self, interface: 's'):
        trace('Scan', interface)
        forget_if_down()
        loop = asyncio.get_event_loop()
        loop.call_later(1.0, self.ScanResultsSignal, IFACE)
        # wpa_supplicant joins a known network it finds on its own; Nickel
        # only waits for the state to change.
        if not STATE.connected and STATE.networks:
            loop.call_later(1.5, lambda: loop.create_task(connect(self)))

    @method(name='ScanResults')
    def ScanResultsMethod(self, interface: 's') -> 'aa{sv}':
        # aa{sv} as dhcpcd-dbus sends it (its introspection says a(a{sv})).
        trace('ScanResultsMethod')
        return [{
            'BSSID': Variant('s', BSSID),
            'Frequency': Variant('i', FREQ),
            'BeaconInterval': Variant('i', 100),
            'Capabilities': Variant('i', 0x431),
            'Quality': Variant('i', 0),
            'Noise': Variant('i', 0),
            'Level': Variant('i', LEVEL),
            'Flags': Variant('s', '[WPA2-PSK-CCMP][ESS]'),
            'SSID': Variant('s', SSID),
        }]

    @method()
    def ListNetworks(self, interface: 's') -> 'a(isss)':
        trace('ListNetworks')
        flags = '[CURRENT]' if STATE.connected else ''
        return [[i, s, 'any', flags] for i, s in STATE.networks.items()]

    @method()
    def AddNetwork(self, interface: 's') -> 'i':
        trace('AddNetwork')
        new = max(STATE.networks, default=-1) + 1
        STATE.networks[new] = ''
        return new

    @method()
    def RemoveNetwork(self, interface: 's', id: 'i'):
        STATE.networks.pop(id, None)

    @method()
    def EnableNetwork(self, interface: 's', id: 'i'):
        log('EnableNetwork', id)
        asyncio.get_event_loop().create_task(connect(self))

    @method()
    def EnableAllNetworks(self, interface: 's'):
        log('EnableAllNetworks')
        asyncio.get_event_loop().create_task(connect(self))

    @method()
    def DisableNetwork(self, interface: 's', id: 'i'):
        pass

    @method()
    def SelectNetwork(self, interface: 's', id: 'i'):
        log('SelectNetwork', id)
        asyncio.get_event_loop().create_task(connect(self))

    @method()
    def GetNetwork(self, interface: 's', id: 'i', parameter: 's') -> 's':
        trace('GetNetwork', id, parameter)
        # Values as wpa_supplicant's GET_NETWORK gives them.
        values = {'ssid': '"%s"' % STATE.networks.get(id, SSID), 'key_mgmt': 'WPA-PSK',
                  'scan_ssid': '0', 'psk': '*', 'priority': '0', 'disabled': '0'}
        return values.get(parameter, '')

    @method()
    def SetNetwork(self, interface: 's', id: 'i', parameter: 's', value: 's'):
        trace('SetNetwork')
        if parameter == 'ssid':
            STATE.networks[id] = value.strip('"')

    @method()
    def SaveConfig(self, interface: 's'):
        trace('SaveConfig')
        pass

    @method()
    def Disconnect(self, interface: 's'):
        log('Disconnect')
        disconnect(self)

    @method()
    def Reconfigure(self, interface: 's'):
        pass

    @method()
    def Reassociate(self, interface: 's'):
        log('Reassociate')
        asyncio.get_event_loop().create_task(connect(self))

    @method()
    def WpsPbc(self, interface: 's'):
        pass

    @method()
    def Terminate(self, interface: 's'):
        log('Terminate')
        disconnect(self)

    @signal()
    def Event(self) -> 'a{sv}':
        return self.iface_info()

    @signal()
    def StatusChanged(self) -> 's':
        return 'connected' if STATE.connected else 'disconnected'

    @signal(name='ScanResults')
    def ScanResultsSignal(self, interface) -> 's':
        return interface

    @signal()
    def WpaStatus(self) -> 'a{sa{ss}}':
        return {IFACE: wpa_status()}

    @signal()
    def SignalStrength(self) -> 'a{sa{ss}}':
        return {IFACE: wpa_status()}


def wpa_status():
    if not STATE.connected:
        return {'wpa_state': 'DISCONNECTED', 'address': '02:00:00:00:00:02'}
    net, _ = address()
    return {
        'bssid': BSSID, 'freq': str(FREQ), 'ssid': SSID, 'id': '0', 'mode': 'station',
        'pairwise_cipher': 'CCMP', 'group_cipher': 'CCMP', 'key_mgmt': 'WPA2-PSK',
        'wpa_state': 'COMPLETED', 'ip_address': str(net.ip) if net else '',
        'address': '02:00:00:00:00:02', 'level': str(LEVEL),
    }


async def connect(service):
    if STATE.connected:
        service.WpaStatus()
        service.Event()
        service.StatusChanged()
        return
    log('connecting to', SSID)
    service.StatusChanged()  # still "disconnected": Nickel shows connecting
    await asyncio.sleep(0.5)
    bring_up()
    STATE.connected = True
    wpa_event('<2>CTRL-EVENT-CONNECTED - Connection to %s completed [id=0 id_str=]' % BSSID)
    service.WpaStatus()
    await asyncio.sleep(0.5)
    service.Event()
    service.StatusChanged()
    log('connected')


def forget_if_down():
    """Nickel switches Wi-Fi off without telling wpa_supplicant (it kills it
    and takes wlan0 down): treat a link that is down as disconnected."""
    if STATE.connected and not link_up():
        STATE.connected = False
        log('wlan0 is down: disconnected')


def disconnect(service):
    if not STATE.connected:
        return
    STATE.connected = False
    wpa_event('<3>CTRL-EVENT-DISCONNECTED bssid=%s reason=3 locally_generated=1' % BSSID)
    service.WpaStatus()
    service.Event()
    service.StatusChanged()
    log('disconnected')


# wpa_supplicant control socket ------------------------------------------

WPA = None
ATTACHED = set()


def wpa_event(text):
    for peer in list(ATTACHED):
        try:
            WPA.sendto(text.encode(), peer)
        except OSError:
            ATTACHED.discard(peer)


def wpa_reply(cmd, peer):
    words = cmd.split()
    head = words[0] if words else ''
    if head == 'PING':
        return 'PONG\n'
    if head == 'ATTACH':
        ATTACHED.add(peer)
        return 'OK\n'
    if head == 'DETACH':
        ATTACHED.discard(peer)
        return 'OK\n'
    if head == 'STATUS':
        return ''.join('%s=%s\n' % kv for kv in wpa_status().items())
    if head == 'SCAN':
        asyncio.get_event_loop().call_later(1.0, wpa_event, '<2>CTRL-EVENT-SCAN-RESULTS ')
        return 'OK\n'
    if head == 'SCAN_RESULTS':
        return ('bssid / frequency / signal level / flags / ssid\n'
                '%s\t%d\t%d\t[WPA2-PSK-CCMP][ESS]\t%s\n' % (BSSID, FREQ, LEVEL, SSID))
    if head == 'BSS':
        return ('id=0\nbssid=%s\nfreq=%d\nbeacon_int=100\ncapabilities=0x0431\nqual=0\n'
                'noise=0\nlevel=%d\nflags=[WPA2-PSK-CCMP][ESS]\nssid=%s\n' % (BSSID, FREQ, LEVEL, SSID))
    if head == 'LIST_NETWORKS':
        flags = '[CURRENT]' if STATE.connected else ''
        return 'network id / ssid / bssid / flags\n' + ''.join(
            '%d\t%s\tany\t%s\n' % (i, s, flags) for i, s in STATE.networks.items())
    if head == 'ADD_NETWORK':
        new = max(STATE.networks, default=-1) + 1
        STATE.networks[new] = ''
        return '%d\n' % new
    if head == 'GET_NETWORK' and len(words) >= 3 and words[2] == 'ssid':
        return '"%s"' % STATE.networks.get(int(words[1]), SSID)
    if head == 'SIGNAL_POLL':
        return 'RSSI=%d\nLINKSPEED=72\nNOISE=9999\nFREQUENCY=%d\n' % (LEVEL, FREQ)
    if head in ('SELECT_NETWORK', 'ENABLE_NETWORK', 'REASSOCIATE', 'RECONNECT'):
        asyncio.get_event_loop().create_task(connect(SERVICE))
        return 'OK\n'
    if head in ('DISCONNECT', 'TERMINATE'):
        disconnect(SERVICE)
        return 'OK\n'
    return 'OK\n'


def on_wpa():
    try:
        data, peer = WPA.recvfrom(4096)
    except OSError:
        return
    cmd = data.decode(errors='replace').strip()
    if os.environ.get('VM_WIFI_TRACE'):
        log('wpa <-', cmd)
    try:
        WPA.sendto(wpa_reply(cmd, peer).encode(), peer)
    except OSError as e:
        log('wpa reply failed:', e)


SERVICE = None


async def main():
    global WPA, SERVICE
    rename_interface()
    os.makedirs(WPA_DIR, exist_ok=True)
    path = os.path.join(WPA_DIR, IFACE)
    if os.path.exists(path):
        os.unlink(path)
    WPA = socket.socket(socket.AF_UNIX, socket.SOCK_DGRAM)
    WPA.bind(path)
    WPA.setblocking(False)
    asyncio.get_event_loop().add_reader(WPA.fileno(), on_wpa)

    # The volume may still hold the socket of the last run; retry until the
    # VM's new dbus-daemon answers.
    while True:
        try:
            bus = await MessageBus(bus_address='unix:path=/run/dbus/system_bus_socket',
                                   bus_type=BusType.SYSTEM).connect()
            break
        except (OSError, Exception):
            await asyncio.sleep(0.5)
    SERVICE = Dhcpcd()
    bus.export('/name/marples/roy/dhcpcd', SERVICE)
    await bus.request_name('name.marples.roy.dhcpcd')
    open(READY, 'w').close()
    log('owns name.marples.roy.dhcpcd; wpa socket', path)
    await asyncio.Event().wait()


if __name__ == '__main__':
    sys.exit(asyncio.run(main()))
