use crate::{application::RootViewport, prelude::*};
use std::time::{Duration, Instant};

use winit::{
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    window::Window,
};

pub mod gui_overlay;
pub mod input;

use crate::render_context::RenderContext;

pub struct AppWindow {
    render_ctx: RenderContext,
    root_viewport: RootViewport,
    // Not used, but needs to be kept alive
    _window: Window,
}

impl AppWindow {
    pub fn new() -> (Self, EventLoop<()>) {
        let event_loop = winit::event_loop::EventLoop::new();
        let window = {
            let mut builder = winit::window::WindowBuilder::new();
            builder = builder.with_title("Blackjack");
            builder.build(&event_loop).expect("Could not build window")
        };

        let window_size = window.inner_size();
        let scale_factor = window.scale_factor();
        let render_ctx = RenderContext::new(&window);
        let root_viewport = RootViewport::new(
            &render_ctx.renderer,
            UVec2::new(window_size.width, window_size.height),
            scale_factor,
            render_ctx.texture_format,
        );

        (
            AppWindow {
                _window: window,
                render_ctx,
                root_viewport,
            },
            // Event loop returned separately because we want to keep creating
            // &mut references to AppWindow after the event loop starts
            event_loop,
        )
    }

    fn on_main_events_cleared(&mut self) {
        // Record the frame time at the start of the frame.
        let frame_start_time = Instant::now();

        self.root_viewport.update(&mut self.render_ctx);
        self.root_viewport.render(&mut self.render_ctx);

        // Sleep for the remaining time to cap at 60Hz
        let elapsed = Instant::now().duration_since(frame_start_time);
        //println!("elapsed {:?}", elapsed);
        let remaining = Duration::from_secs_f32(1.0 / 60.0).saturating_sub(elapsed);
        //println!("remaining {:?}", remaining);
        spin_sleep::sleep(remaining);
    }

    pub fn run_app(mut self, event_loop: EventLoop<()>) {
        self.root_viewport.setup(&mut self.render_ctx);

        event_loop.run(move |event, _, control| {
            match event {
                Event::WindowEvent { ref event, .. } => {
                    match event {
                        // Close requested
                        WindowEvent::CloseRequested => {
                            println!("Close requested");
                            *control = winit::event_loop::ControlFlow::Exit;
                        }

                        // Resize
                        WindowEvent::Resized(ref new_size) => {
                            self.render_ctx.on_resize(new_size.width, new_size.height);
                        }

                        _ => {}
                    }
                }
                // Main events cleared
                Event::MainEventsCleared => self.on_main_events_cleared(),
                _ => {}
            }
            self.root_viewport.on_winit_event(event);
        });
    }
}
