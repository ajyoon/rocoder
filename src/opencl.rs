use ocl::{
    self,
    builders::ProgramBuilder,
    flags::{CommandQueueProperties, MemFlags},
    ProQue, SpatialDims,
};
use stopwatch::Stopwatch;

pub struct OpenClProgram {
    kernel_program: ProQue,
}

impl OpenClProgram {
    pub fn new(src: String, buffer_size: usize) -> Self {
        let sw = Stopwatch::start_new();
        let mut program_builder = ProgramBuilder::new();
        program_builder.cmplr_def("LEN", buffer_size as i32);
        program_builder.cmplr_opt("-cl-fast-relaxed-math");
        program_builder.source(src);
        let kernel_program = ProQue::builder()
            .prog_bldr(program_builder)
            .dims(SpatialDims::One(buffer_size))
            .queue_properties(CommandQueueProperties::new().out_of_order())
            .build()
            .unwrap();
        info!("Created ProQue in {:?}", sw.elapsed());
        println!(
            "{}",
            kernel_program
                .queue()
                .info(ocl::enums::CommandQueueInfo::Properties)
                .unwrap()
        );

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
                .copy_host_slice(&frequency_bins)
                .flags(MemFlags::new().read_only())
                .build()?
        };
        let out_buf = unsafe {
            pro_que
                .buffer_builder::<f32>()
                .use_host_slice(&frequency_bins)
                .build()?
        };
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
        unsafe { out_buf.map().enq()? };

        //debug!("applied kernel in {:?}", sw.elapsed());
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test_utils::*;
    use std::path::PathBuf;
    extern crate test;
    use test::Bencher;

    #[test]
    #[ignore] // slow
    fn opencl_experiment() {
        let kernel_src = r#"
            __kernel void transform(__global float const* const in_buf, __global float* const out_buf, 
                                    __private uint elapsed_ms) {
              uint idx = get_global_id(0);
              out_buf[idx] = in_buf[idx] + (float) idx;
            }
        "#.to_string();

        let program = OpenClProgram::new(kernel_src, 5);
        let mut buffer = vec![10.0; 5];
        program.apply_fft_transform(&mut buffer, 123).unwrap();
        assert_almost_eq_by_element(buffer.clone(), vec![10.0, 11.0, 12.0, 13.0, 14.0]);
        assert!(false);
    }

    #[bench]
    fn benchmark_kernel(b: &mut Bencher) {
        // 156 us for 32768
        let buffer_size = 32768;
        let kernel_src = std::fs::read_to_string(PathBuf::from("test.cl")).unwrap();
        let program = OpenClProgram::new(kernel_src, buffer_size);

        let mut buffer = vec![0.0; buffer_size];
        b.iter(|| {
            let _ = test::black_box(1);
            program.apply_fft_transform(&mut buffer, 123).unwrap();
        });
    }
}
