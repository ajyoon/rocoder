__kernel void transform(__global float const* const in_buf, __global float* const out_buf, 
                        __private uint len, __private uint sample_rate, __private uint dest_sample_pos) {
  uint idx = get_global_id(0);
  uint dest_elapsed_s = dest_sample_pos / sample_rate;
  
  out_buf[idx] = in_buf[(idx - (dest_elapsed_s * 4)) % len];
}
