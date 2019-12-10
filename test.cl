typedef struct {float re; float im;} complex_t;
__kernel void transform(__global complex_t const* in_buf, __global complex_t* out_buf, 
                        __private uint elapsed_ms) {
  uint idx = get_global_id(0);
  if (idx % 4) {
    out_buf[idx].re = in_buf[idx].re;
    out_buf[idx].im = in_buf[idx].im;
  } else {
    out_buf[idx].re = in_buf[(idx - (elapsed_ms / 250)) % LEN].re;
    out_buf[idx].im = in_buf[(idx - (elapsed_ms / 250)) % LEN].im;
  }
}
