__kernel void transform(__global float const* const in_buf, __global float* const out_buf, 
                        __private uint elapsed_ms) {
  uint idx = get_global_id(0);
  
  out_buf[idx] = in_buf[(idx - (elapsed_ms / 250)) % LEN];
}
