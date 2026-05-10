use crate::core::Engine;

#[test]
pub fn server_initialization() {
    let mut app = Engine::headless();

    app.finish();
    app.cleanup();
    // run for 60 frames

    for _ in 0..60 {
        app.update();
    }
}