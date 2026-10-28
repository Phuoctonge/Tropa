#[tokio::main]
async fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if !(2..=4).contains(&args.len()) || (args.len() == 4 && args[3] != "--refresh") {
        eprintln!(
            "Usage: tropa-warp-probe <absolute-data-directory> <pinned-mihomo-executable> [source-bound-runtime-directory [--refresh]]"
        );
        std::process::exit(2);
    }
    let result = if args.len() == 4 {
        app_lib::refresh_warp_discovery_probe(args[0].clone().into(), args[1].clone().into(), args[2].clone().into())
            .await
    } else if args.len() == 3 {
        app_lib::run_warp_discovery_probe(args[0].clone().into(), args[1].clone().into(), args[2].clone().into()).await
    } else {
        app_lib::run_warp_transport_probe(args[0].clone().into(), args[1].clone().into()).await
    };
    match result {
        Ok(report) => println!(
            "{}",
            serde_json::to_string_pretty(&report).expect("public probe report")
        ),
        Err(error) => {
            eprintln!("WARP probe failed: {error}");
            std::process::exit(1);
        }
    }
}
