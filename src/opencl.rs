use ocl::{
    self,
    builders::ProgramBuilder,
    flags::{CommandQueueProperties, MemFlags},
    OclPrm, ProQue, SpatialDims,
};
use rustfft::num_complex::Complex32 as FftComplex;
use stopwatch::Stopwatch;

/// copied from num_complex because orphan rule :^)
#[derive(PartialEq, Eq, Copy, Clone, Hash, Debug, Default)]
#[repr(C)]
pub struct Complex<T> {
    /// Real portion of the complex number
    pub re: T,
    /// Imaginary portion of the complex number
    pub im: T,
}

unsafe impl OclPrm for Complex<f32> {}

impl std::convert::From<FftComplex> for Complex<f32> {
    fn from(obj: FftComplex) -> Self {
        Complex {
            re: obj.re,
            im: obj.im,
        }
    }
}

impl std::convert::From<Complex<f32>> for FftComplex {
    fn from(obj: Complex<f32>) -> Self {
        FftComplex {
            re: obj.re,
            im: obj.im,
        }
    }
}

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
        info!(
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
        fft_result: &mut Vec<FftComplex>,
        elapsed_ms: u32,
    ) -> Result<(), ocl::Error> {
        let pro_que = &self.kernel_program;
        debug_assert!(pro_que.dims()[0] == fft_result.len());
        //let sw = Stopwatch::start_new();
        let fft_result_as_ctype: Vec<Complex<f32>> = fft_result
            .iter()
            .map(|r| Complex::<f32>::from(*r))
            .collect();
        let in_buf = pro_que
            .buffer_builder::<Complex<f32>>()
            .copy_host_slice(&fft_result_as_ctype)
            .flags(MemFlags::new().read_only())
            .build()?;
        let out_buf = pro_que.create_buffer::<Complex<f32>>()?;
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

        let mut out_vec = vec![Complex::<f32> { re: 0.0, im: 0.0 }; out_buf.len()];
        out_buf.read(&mut out_vec).enq()?;

        for i in 0..fft_result.len() {
            fft_result[i] = FftComplex::from(out_vec[i]);
        }

        // debug!("applied kernel in {:?}", sw.elapsed());
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
            typedef struct {float re; float im;} complex_t;
            __kernel void transform(__global complex_t const* in_buf, __global complex_t* out_buf, 
                                    __private uint elapsed_ms) {
              uint idx = get_global_id(0);
              out_buf[idx].re = in_buf[idx].re + (float) idx;
              out_buf[idx].im = in_buf[idx].im + (float) idx;
            }
        "#
        .to_string();

        let program = OpenClProgram::new(kernel_src, 5);
        let mut buffer = vec![FftComplex { re: 9.0, im: 10.0 }; 5];
        program.apply_fft_transform(&mut buffer, 123).unwrap();
        assert_almost_eq(buffer[0].re, 9.0);
        assert_almost_eq(buffer[0].im, 10.0);
        assert_almost_eq(buffer[1].re, 10.0);
        assert_almost_eq(buffer[1].im, 11.0);
    }

    #[bench]
    fn benchmark_kernel(b: &mut Bencher) {
        // 156 us for 32768
        let buffer_size = 32768;
        let kernel_src = std::fs::read_to_string(PathBuf::from("test.cl")).unwrap();
        let program = OpenClProgram::new(kernel_src, buffer_size);

        let mut buffer = vec![FftComplex { re: 10.0, im: 9.0 }; buffer_size];
        b.iter(|| {
            let _ = test::black_box(1);
            program.apply_fft_transform(&mut buffer, 123).unwrap();
        });
    }
}
