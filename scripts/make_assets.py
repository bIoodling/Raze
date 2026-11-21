"""Rasterize the reference-derived vector mark for installer resources (CI only)."""
import struct
from pathlib import Path

out=Path("installer/assets")
out.mkdir(parents=True,exist_ok=True)
points=[(8,0),(84,0),(90,8),(74,39),(60,39),(76,65),(53,65),(37,39),(24,39),(11,65),(0,65),(14,30),(21,26),(59,26),(65,14),(16,14)]
def inside(x,y):
    result=False
    j=len(points)-1
    for i,(xi,yi) in enumerate(points):
        xj,yj=points[j]
        if (yi>y)!=(yj>y) and x<(xj-xi)*(y-yi)/(yj-yi)+xi: result=not result
        j=i
    return result
def pixels(w,h,size):
    ox=(w-size)/2; sy=size*65/90; oy=(h-sy)/2
    return b"".join(bytes((241,38,173,255)) if inside((x-ox)*90/size,(y-oy)*90/size) else bytes((16,9,8,255)) for y in range(h) for x in range(w))
def bmp(w,h,size):
    p=pixels(w,h,size)
    return b"BM"+struct.pack("<IHHI",54+len(p),0,0,54)+struct.pack("<IiiHHIIIIII",40,w,-h,1,32,0,len(p),0,0,0,0)+p
out.joinpath("wizard.bmp").write_bytes(bmp(164,314,126))
out.joinpath("small.bmp").write_bytes(bmp(55,55,44))
p=pixels(64,64,56)
rows=b"".join(p[y*64*4:(y+1)*64*4] for y in range(63,-1,-1))
dib=struct.pack("<IiiHHIIIIII",40,64,128,1,32,0,len(rows),0,0,0,0)+rows+bytes(64*8)
ico=struct.pack("<HHH",0,1,1)+struct.pack("<BBBBHHII",64,64,0,0,1,32,len(dib),22)+dib
out.joinpath("raze.ico").write_bytes(ico)
