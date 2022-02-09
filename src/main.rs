use packet_tracer_generator::App;

fn main() {
    let app = App::new();

    // Commands here

    for (dev_name, commands) in app.to_commands() {
        println!("== {} ==\n{}\n", dev_name, commands);
    }
}
