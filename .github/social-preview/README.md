# Social preview

`social-preview.png` (1280×640) is the image shown when the repository is linked.
GitHub does not take it from the repository: upload it under
Settings → General → Social preview.

`generate.py` recreates it (Python with Pillow; uses the macOS Helvetica font):

```sh
cd .github/social-preview && python3 -I generate.py
```
