#include "adapter.h"
#include <pappl/pappl.h>
#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#ifndef PAPPL_DOCUMENT_POLICY_API
#error "Build the pinned PAPPL with patches/pappl-1.4.9-document-policy.patch"
#endif

typedef struct {
  void *app;
  const lt_config *config;
  char media[3][64];
} bridge;

static bool report(pappl_job_t *job, int result) {
  if (!result) return true;
  papplJobSetMessage(job, "Input rejected or print outcome uncertain; inspect local journal. No automatic retry.");
  papplJobSetReasons(job, PAPPL_JREASON_DOCUMENT_UNPRINTABLE_ERROR, PAPPL_JREASON_NONE);
  return false;
}

static bool supported_options(pappl_job_t *job, pappl_pr_options_t *o) {
  return o->copies == 1 && o->print_scaling == PAPPL_SCALING_NONE &&
         o->orientation_requested == IPP_ORIENT_NONE &&
         papplJobGetAttribute(job,"page-ranges") == NULL;
}

static bool start_job(pappl_job_t *job, pappl_pr_options_t *o, pappl_device_t *d) {
  (void)o; (void)d;
  return report(job, 1); /* No streaming or image conversion inputs. */
}

static bool start_page(pappl_job_t *job, pappl_pr_options_t *o, pappl_device_t *d, unsigned page) {
  (void)o; (void)d; (void)page;
  return report(job,1);
}

static bool write_line(pappl_job_t *job, pappl_pr_options_t *o, pappl_device_t *d, unsigned y, const unsigned char *line) {
  (void)o; (void)d; (void)y; (void)line;
  return report(job,1);
}

static bool end_page(pappl_job_t *job, pappl_pr_options_t *o, pappl_device_t *d, unsigned p) {
  (void)o; (void)d; (void)p;
  return report(job,1);
}

static bool end_job(pappl_job_t *job, pappl_pr_options_t *o, pappl_device_t *d) {
  (void)o; (void)d;
  return report(job,1);
}

static bool raw_file(pappl_job_t *j, pappl_pr_options_t *o, pappl_device_t *d) {
  (void)o; (void)d;
  return report(j, 1);
}

static bool pdf_file(pappl_job_t *job, pappl_device_t *d, void *data) {
  (void)d;
  if (papplJobIsCanceled(job) || papplJobGetCopies(job) != 1) return report(job, 1);
  const char *filename = papplJobGetFilename(job);
  pappl_pr_options_t *o=papplJobCreatePrintOptions(job,1,false);
  if (!o) return false;
  int result=filename && supported_options(job,o) ? lt_rs_pdf(data, filename, strlen(filename), (uint32_t)o->media.size_width,
                                (uint32_t)o->media.size_length, o->print_scaling == PAPPL_SCALING_NONE) : 1;
  papplJobDeletePrintOptions(o);
  return report(job,result);
}

static bool device_open(pappl_device_t *d, const char *uri, const char *id) {
  (void)d; (void)uri; (void)id;
  return true;
}
static void device_close(pappl_device_t *d) { (void)d; }
static ssize_t device_write(pappl_device_t *d, const void *b, size_t n) {
  (void)d; (void)b; (void)n;
  return -1; /* Transport bytes never pass through PAPPL. */
}

static bool driver(pappl_system_t *sys, const char *name, const char *uri,
                   const char *id, pappl_pr_driver_data_t *d, ipp_t **attrs, void *data) {
  (void)sys; (void)name; (void)uri; (void)id;
  bridge *b = data;
  d->extension = b;
  d->rstartjob_cb = start_job; d->rstartpage_cb = start_page;
  d->rwriteline_cb = write_line; d->rendpage_cb = end_page; d->rendjob_cb = end_job;
  d->printfile_cb = raw_file;
  d->format = "application/vnd.letratag.internal";
  papplCopyString(d->make_and_model, b->config->model, sizeof(d->make_and_model));
  d->kind = PAPPL_KIND_LABEL | PAPPL_KIND_ROLL;
  d->ppm = 1; /* Scheduling hint, not a measured device speed. */
  d->num_resolution = 1;
  d->x_resolution[0] = d->y_resolution[0] = d->x_default = d->y_default = (int)b->config->dpi;
  d->raster_types = PAPPL_PWG_RASTER_TYPE_SGRAY_8 | PAPPL_PWG_RASTER_TYPE_BLACK_8;
  d->force_raster_type = PAPPL_PWG_RASTER_TYPE_SGRAY_8;
  d->color_supported = d->color_default = PAPPL_COLOR_MODE_MONOCHROME;
  d->sides_supported = d->sides_default = PAPPL_SIDES_ONE_SIDED;
  d->scaling_default = PAPPL_SCALING_NONE;
  d->orient_default = IPP_ORIENT_NONE;
  d->quality_default = IPP_QUALITY_NORMAL;
  d->borderless = true;
  d->num_media = 3;
  for (int i=0;i<3;i++) d->media[i] = b->media[i];
  d->num_source=1; d->source[0]="main-roll";
  d->num_type=1; d->type[0]="continuous";
  papplCopyString(d->media_default.size_name,b->media[0],sizeof(d->media_default.size_name));
  d->media_default.size_width=(int)b->config->media_width;
  d->media_default.size_length=(int)b->config->default_length;
  papplCopyString(d->media_default.source,"main-roll",sizeof(d->media_default.source));
  papplCopyString(d->media_default.type,"continuous",sizeof(d->media_default.type));
  d->media_ready[0]=d->media_default;
  *attrs=ippNew();
  ippAddRange(*attrs,IPP_TAG_PRINTER,"copies-supported",1,1);
  ippAddRange(*attrs,IPP_TAG_PRINTER,"job-k-octets-supported",0,8192);
  ippAddBoolean(*attrs,IPP_TAG_PRINTER,"page-ranges-supported",false);
  ippAddInteger(*attrs,IPP_TAG_PRINTER,IPP_TAG_ENUM,"orientation-requested-supported",IPP_ORIENT_NONE);
  ippAddString(*attrs,IPP_TAG_PRINTER,IPP_TAG_KEYWORD,"print-scaling-supported",NULL,"none");
  return true;
}

int lt_serve(void *app, const lt_config *config, const char *spool, int port) {
  bridge b={.app=app,.config=config};
  snprintf(b.media[0],64,"custom_default_%ux%umm",config->media_width/100,config->default_length/100);
  snprintf(b.media[1],64,"custom_min_%ux%umm",config->media_width/100,config->min_length/100);
  snprintf(b.media[2],64,"custom_max_%ux%umm",config->media_width/100,config->max_length/100);
  pappl_system_t *s=papplSystemCreate(PAPPL_SOPTIONS_NO_TLS,"LetraTag local development",port,
                                    NULL,spool,"-",PAPPL_LOGLEVEL_ERROR,NULL,false);
  if (!s) return 1;
  if (!papplSystemSetDocumentPolicy(s,"application/pdf",config->max_job_bytes)) {
    papplSystemDelete(s);return 4;
  }
  papplSystemSetDNSSDName(s,NULL);
  papplSystemSetHostName(s,"localhost");
  papplSystemSetMaxClients(s,8);
  papplSystemSetMaxImageSize(s,16*1024*1024,8192,8192);
  if (!papplSystemAddListeners(s,"127.0.0.1")) {papplSystemDelete(s);return 2;}
  papplDeviceAddScheme("ltlocal",PAPPL_DEVTYPE_CUSTOM_LOCAL,NULL,device_open,device_close,NULL,device_write,NULL,NULL);
  pappl_pr_driver_t entry={"ltlocal",config->model,NULL,NULL};
  papplSystemSetPrinterDrivers(s,1,&entry,NULL,NULL,driver,&b);
  papplSystemAddMIMEFilter(s,"application/pdf","application/vnd.letratag.internal",pdf_file,app);
  pappl_printer_t *p=papplPrinterCreate(s,1,"letratag","ltlocal",NULL,"ltlocal://local");
  if (!p) {papplSystemDelete(s);return 3;}
  papplPrinterSetDNSSDName(p,NULL);
  papplPrinterSetMaxActiveJobs(p,16);
  papplPrinterSetMaxCompletedJobs(p,16);
  papplPrinterSetMaxPreservedJobs(p,0);
  papplSystemRun(s);
  /* PAPPL 1.4 has detached clients, not a public worker-drain guarantee.
     Never release borrowed Rust/stack state while those clients may exist.
     This server entry point owns the process, including its shutdown. */
  _Exit(0);
}
