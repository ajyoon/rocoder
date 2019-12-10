typedef struct {float re; float im;} complex_t;
__kernel void transform(__global complex_t const* in_buf, __global complex_t* out_buf, 
                        __private uint elapsed_ms) {
  uint idx = get_global_id(0);
  // low pass
  bool filter = idx > LEN - (LEN / 100);

  // high pass
  /* int dist_from_center = abs((LEN / 2) - (int) idx); */
  /* bool filter = dist_from_center < LEN / 3; */
  
  if (filter) {
    out_buf[idx].re = in_buf[idx].re;
    out_buf[idx].im = in_buf[idx].im;
  }
}
