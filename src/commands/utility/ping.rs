#[command]
#[short_desc("Check if I'm online")]
#[long_desc(
    "Check if I'm online.\n\
    The latency indicates how fast I receive messages from Discord."
)]
#[aliases("p")]
fn ping(_ctx: &mut (), _msg: &(), _args: ()) -> () {
    ()
}
