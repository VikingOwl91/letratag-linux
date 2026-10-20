#include <cups/raster.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>
int main(void) {
  cups_page_header2_t h;
  memset(&h,0,sizeof(h));
  h.HWResolution[0]=h.HWResolution[1]=160;
  h.PageSize[0]=34;h.PageSize[1]=142;
  h.cupsPageSize[0]=12.0f*72/25.4f;h.cupsPageSize[1]=50.0f*72/25.4f;
  h.cupsWidth=76;h.cupsHeight=315;
  h.cupsBitsPerColor=h.cupsBitsPerPixel=8;
  h.cupsBytesPerLine=76;h.cupsColorSpace=CUPS_CSPACE_SW;
  h.cupsColorOrder=CUPS_ORDER_CHUNKED;h.cupsNumColors=1;
  cups_raster_t *r=cupsRasterOpen(STDOUT_FILENO,CUPS_RASTER_WRITE_PWG);
  if(!r || !cupsRasterWriteHeader2(r,&h))return 1;
  for(unsigned y=0;y<h.cupsHeight;y++) {
    unsigned char line[76];memset(line,255,sizeof(line));
    if(y>=32&&y<280)for(unsigned x=31;x<45;x++)if((y/10)%2==0)line[x]=0;
    if(cupsRasterWritePixels(r,line,sizeof(line))!=sizeof(line))return 2;
  }
  cupsRasterClose(r);return 0;
}
