#include "../crates/letratag-printer/native/adapter.h"
#include <stdio.h>
#include <stdlib.h>
#include <inttypes.h>

/* A generic sink proves the framework preserves the complete document bytes. */
int lt_rs_pdf(void *app, const char *path, size_t len, uint32_t width, uint32_t length, int no_scaling) {
  (void)app;(void)len;(void)width;(void)length;(void)no_scaling;
  FILE *f=fopen(path,"rb");
  if(!f)return 1;
  uint64_t hash=UINT64_C(14695981039346656037);
  size_t size=0;
  int c;
  while((c=fgetc(f))!=EOF){hash^=(unsigned char)c;hash*=UINT64_C(1099511628211);size++;}
  int result=ferror(f);fclose(f);
  if(result)return 1;
  printf("RECEIVED %zu %016" PRIx64 "\n",size,hash);fflush(stdout);
  return 0;
}

int main(int argc,char **argv) {
  if(argc==2) {
    FILE *f=fopen(argv[1],"rb");if(!f)return 1;
    uint64_t hash=UINT64_C(14695981039346656037);size_t n=0;int c;
    while((c=fgetc(f))!=EOF){hash^=(unsigned char)c;hash*=UINT64_C(1099511628211);n++;}
    fclose(f);printf("RECEIVED %zu %016" PRIx64 "\n",n,hash);return 0;
  }
  if(argc!=3)return 2;
  lt_config config={.dpi=200,.media_width=1200,.min_length=1000,.max_length=20000,
    .default_length=5000,.max_job_bytes=8*1024*1024,.model="Generic PDF policy test sink"};
  return lt_serve(NULL,&config,argv[1],atoi(argv[2]));
}
