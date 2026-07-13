fn main() {
    // Skeleton entry point: emit the health payload the backend will later serve
    // over HTTP. Proves the binary builds and links on every release target.
    println!("{}", provreq::health_json());
}
