# Fake Wi-Fi for the Nickel VM (vm/wifi/wifi.py), native on the host CPU.
FROM docker.io/library/python:3.12-slim
RUN apt-get update && apt-get install -y --no-install-recommends iproute2 \
    && rm -rf /var/lib/apt/lists/* \
    && pip install --no-cache-dir dbus-next==0.2.3
COPY wifi.py /usr/local/bin/vm-wifi
CMD ["python", "-I", "/usr/local/bin/vm-wifi"]
