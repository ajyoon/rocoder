typedef struct {float re; float im;} complex_t;
__kernel void transform(__global complex_t const* in_buf, __global complex_t* out_buf, 
                        __private uint elapsed_ms) {
  uint idx = get_global_id(0);
  out_buf[idx].re = in_buf[(idx + (elapsed_ms / 250)) % LEN].re;
}
