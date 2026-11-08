from PIL import Image, ImageDraw, ImageFont
W,H=1280,640
im=Image.new("RGB",(W,H),"#f4f1ea"); d=ImageDraw.Draw(im)
def f(sz,bold=False):
    return ImageFont.truetype("/System/Library/Fonts/Helvetica.ttc",sz,index=1 if bold else 0)
d.rectangle([0,0,W,14],fill="#1c1c1c")
d.text((80,90),"NickelInkSync",font=f(104,True),fill="#1c1c1c")
d.text((84,230),"Sync the reading progress of sideloaded",font=f(40),fill="#333")
d.text((84,282),"ebooks between Kobo, Tolino and KOReader.",font=f(40),fill="#333")
x=80
for name in ["Kobo","Tolino","KOReader","Kindle"]:
    d.rounded_rectangle([x,380,x+270,560],radius=18,outline="#1c1c1c",width=4,fill="#fff")
    d.text((x+24,396),name,font=f(34,True),fill="#1c1c1c")
    d.text((x+24,448),"Words of Radiance",font=f(23),fill="#444")
    d.text((x+24,480),"62 %",font=f(23,True),fill="#1c1c1c")
    d.rounded_rectangle([x+24,520,x+246,540],radius=10,fill="#ddd")
    d.rounded_rectangle([x+24,520,x+24+int(222*0.62),540],radius=10,fill="#1c1c1c")
    x+=283
d.text((80,590),"Same place in the book, on every device.  github.com/dkuester/NickelInkSync",font=f(26),fill="#666")
im.save("social-preview.png")
