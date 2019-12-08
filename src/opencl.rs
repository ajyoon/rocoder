use ocl::{self, ProQue, SpatialDims};
use stopwatch::Stopwatch;

pub struct OpenClProgram {
    kernel_program: ProQue,
}

impl OpenClProgram {
    pub fn new(src: String, buffer_size: usize) -> Self {
        let sw = Stopwatch::start_new();
        let kernel_program = ProQue::builder()
            .src(src)
            .dims(SpatialDims::One(buffer_size))
            .build()
            .unwrap();
        info!("Created ProQue in {:?}", sw.elapsed());

        Self { kernel_program }
    }

    pub fn apply_fft_transform(
        &self,
        frequency_bins: &mut Vec<f32>,
        elapsed_ms: u32,
    ) -> Result<(), ocl::Error> {
        let pro_que = &self.kernel_program;
        let sw = Stopwatch::start_new();
        let in_buf = unsafe {
            pro_que
                .buffer_builder::<f32>()
                .use_host_slice(&frequency_bins)
                .build()?
        };
        let out_buf = pro_que.create_buffer().unwrap();
        let kernel = pro_que
            .kernel_builder("transform")
            .arg(&in_buf)
            .arg(&out_buf)
            .arg(elapsed_ms)
            .build()
            .unwrap();
        unsafe {
            kernel.enq()?;
        }

        out_buf.read(frequency_bins).enq()?;
        debug!("applied kernel in {:?}", sw.elapsed());
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test_utils::*;

    #[test]
    #[ignore] // slow
    fn opencl_experiment() {
        let kernel_src = r#"
            __kernel void transform(__global float const* const in_buf, __global float* const out_buf, 
                                    __private uint elapsed_ms) {
              uint idx = get_global_id(0);
              out_buf[idx] = (float) idx;
            }
        "#.to_string();

        let program = OpenClProgram::new(kernel_src, 5);
        let mut buffer = vec![0.0; 5];
        program.apply_fft_transform(&mut buffer, 123).unwrap();
        assert_almost_eq_by_element(buffer.clone(), vec![0.0, 1.0, 2.0, 3.0, 4.0]);
        assert!(false);
    }
}
