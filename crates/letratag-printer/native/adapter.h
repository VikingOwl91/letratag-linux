#ifndef LETRATAG_ADAPTER_H
#define LETRATAG_ADAPTER_H
#include <stdint.h>
#include <stddef.h>

/* Rust owns every opaque pointer. C borrows app until lt_serve returns. */
typedef struct {
  uint32_t dpi, media_width, min_length, max_length, default_length;
  size_t max_job_bytes;
  const char *model;
} lt_config;

int lt_serve(void *app, const lt_config *config, const char *spool, int port);
int lt_rs_pdf(void *app, const char *path, size_t len, uint32_t media_width, uint32_t media_length, int no_scaling);
#endif
