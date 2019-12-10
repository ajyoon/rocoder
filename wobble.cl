typedef struct {float re; float im;} complex_t;
__kernel void transform(__global complex_t const* in_buf, __global complex_t* out_buf, 
                        __private uint elapsed_ms) {
  uint idx = get_global_id(0);
  if (idx % 3) {
    out_buf[idx].re = in_buf[idx].re;
    out_buf[idx].im = in_buf[idx].im;
  } else {
    int t = sin(((float) elapsed_ms) / (float) 10.0) * 100;
    out_buf[idx].re = in_buf[(idx + t) % LEN].re;
    out_buf[idx].im = in_buf[(idx + t) % LEN].im;
  }
}
