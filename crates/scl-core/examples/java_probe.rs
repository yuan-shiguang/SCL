//! 打印本机扫描到的 Java 运行时，用来验证探测逻辑。
//!
//! ```bash
//! cargo run -p scl-core --example java_probe
//! ```

fn main() {
    let runtimes = scl_core::java::find_all();
    println!("found {} runtime(s)", runtimes.len());
    for runtime in &runtimes {
        println!(
            "- {}  (Java {}, {})",
            runtime.path.display(),
            runtime.major,
            runtime.vendor
        );
    }
    for major in [8u32, 17, 21] {
        match scl_core::java::find_compatible(major) {
            Some(runtime) => println!("Java {major} -> {}", runtime.path.display()),
            None => println!("Java {major} -> 没有可用运行时"),
        }
    }
}
