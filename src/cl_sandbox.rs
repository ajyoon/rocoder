use ocl::{self, ProQue, SpatialDims};
use std::path::PathBuf;
use stopwatch::Stopwatch;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "cl_sandbox")]
struct Opt {
    #[structopt(parse(from_os_str))]
    kernel_src: PathBuf,
}

fn main() {
    let opt = Opt::from_args();

    let len = 32768;

    let sw = Stopwatch::start_new();
    let pro_que = ProQue::builder()
        .src(std::fs::read_to_string(opt.kernel_src).unwrap())
        .dims(SpatialDims::One(len))
        .build()
        .unwrap();
    println!("Created ProQue in {:?}", sw.elapsed());

    let mut frequency_bins: Vec<f32> = (0..len).map(|i| i as f32).collect();

    let sw = Stopwatch::start_new();
    let in_buf = unsafe {
        pro_que
            .buffer_builder::<f32>()
            .use_host_slice(&frequency_bins)
            .build()
            .unwrap()
    };
    let out_buf = pro_que.create_buffer().unwrap();
    let kernel = pro_que
        .kernel_builder("transform")
        .global_work_size(pro_que.dims().clone())
        .arg(&in_buf)
        .arg(&out_buf)
        .arg((&in_buf).len() as u32)
        .arg(44100)
        .build()
        .unwrap();
    unsafe {
        kernel.enq().unwrap();
    }
    pro_que.queue().finish().unwrap();

    out_buf.read(&mut frequency_bins).enq().unwrap();
    println!("applied kernel in {:?}", sw.elapsed());
    // println!("{:?}", frequency_bins);
}
