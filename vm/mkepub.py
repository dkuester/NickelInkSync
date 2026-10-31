"""Write a small multi-chapter EPUB 3 for VM tests."""
import sys, zipfile, uuid

out, title = sys.argv[1], sys.argv[2]
chapters = int(sys.argv[3]) if len(sys.argv) > 3 else 5
book_id = 'urn:uuid:' + str(uuid.uuid5(uuid.NAMESPACE_URL, title))
para = ('This is test text for the NickelInkSync VM. ' * 12).strip()

def chapter(n):
    body = '\n'.join(f'<p>Chapter {n}, paragraph {i}. {para}</p>' for i in range(1, 41))
    return f'''<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml"><head><title>Chapter {n}</title></head>
<body><h1>Chapter {n}</h1>
{body}
</body></html>'''

with zipfile.ZipFile(out, 'w') as z:
    z.writestr(zipfile.ZipInfo('mimetype'), 'application/epub+zip', compress_type=zipfile.ZIP_STORED)
    z.writestr('META-INF/container.xml', '''<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
<rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles>
</container>''', compress_type=zipfile.ZIP_DEFLATED)
    items = '\n'.join(f'<item id="c{n}" href="c{n}.xhtml" media-type="application/xhtml+xml"/>' for n in range(1, chapters + 1))
    spine = '\n'.join(f'<itemref idref="c{n}"/>' for n in range(1, chapters + 1))
    navlis = '\n'.join(f'<li><a href="c{n}.xhtml">Chapter {n}</a></li>' for n in range(1, chapters + 1))
    z.writestr('OEBPS/content.opf', f'''<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id">
<metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
<dc:identifier id="id">{book_id}</dc:identifier>
<dc:title>{title}</dc:title><dc:creator>NickelInkSync</dc:creator><dc:language>en</dc:language>
<meta property="dcterms:modified">2026-01-01T00:00:00Z</meta>
</metadata>
<manifest>
<item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
{items}
</manifest>
<spine>
{spine}
</spine>
</package>''', compress_type=zipfile.ZIP_DEFLATED)
    z.writestr('OEBPS/nav.xhtml', f'''<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
<head><title>Contents</title></head>
<body><nav epub:type="toc"><ol>
{navlis}
</ol></nav></body></html>''', compress_type=zipfile.ZIP_DEFLATED)
    for n in range(1, chapters + 1):
        z.writestr(f'OEBPS/c{n}.xhtml', chapter(n), compress_type=zipfile.ZIP_DEFLATED)
