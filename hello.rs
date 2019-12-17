fn main() {
    println!("hello world");
}

#[no_mangle]
pub fn foo() -> String {
    "foo".to_owned()
}
