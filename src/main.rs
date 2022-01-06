use iced::{
    button,
    canvas::{event::Status, Cursor, Event, Frame, Program},
    mouse::{Button as MouseBtn, Event as MouseEvt},
    Button, Canvas, Color, Column, Element, Point, Rectangle, Sandbox, Settings, Size, Text,
};
use packet_tracer_generator::{App, Device};
use slotmap::DefaultKey;

const ROUTER_SIZE: Size = Size::new(40., 40.);

fn main() {
    PTGen::run(Settings::default()).unwrap();
}

#[derive(Default)]
struct PTGen {
    app: App,
    interaction: Interaction,

    new_router: button::State,
}

impl Sandbox for PTGen {
    type Message = Message;

    fn new() -> Self {
        Self::default()
    }

    fn title(&self) -> String {
        "SUS".into()
    }

    fn view(&mut self) -> Element<Self::Message> {
        Column::new()
            .push(
                Button::new(&mut self.new_router, Text::new("New router"))
                    .on_press(Message::CreateRouter),
            )
            .push(Canvas::new(PTCanvas { app: &mut self.app }))
            .into()
    }

    fn update(&mut self, msg: Message) {
        use Message::*;

        match msg {
            CreateRouter => drop(self.app.add_device(Device {
                name: "R1".into(),
                ..Default::default()
            })),
            LeftMouseClicked(key) => self.interaction = Interaction::Dragging(key),
            DeselectRouter => self.interaction = Interaction::None,
            LeftMouseReleased => {
                if let Interaction::Dragging(key) = self.interaction {
                    self.interaction = Interaction::Selected(key);
                }
            }
            MouseMoved(pos) => {
                if let Interaction::Dragging(key) = self.interaction {
                    self.app.devices[key].x = pos.x;
                    self.app.devices[key].y = pos.y;
                }
            }
        };
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Message {
    CreateRouter,
    LeftMouseClicked(DefaultKey),
    LeftMouseReleased,
    DeselectRouter,
    MouseMoved(Point),
}

#[derive(Debug)]
pub enum Interaction {
    None,
    Selected(DefaultKey),
    Dragging(DefaultKey),
}

impl Default for Interaction {
    fn default() -> Self {
        Self::None
    }
}

struct PTCanvas<'a> {
    app: &'a mut App,
}

impl Program<Message> for PTCanvas<'_> {
    fn draw(
        &self,
        bounds: iced::Rectangle,
        _cursor: iced::canvas::Cursor,
    ) -> Vec<iced::canvas::Geometry> {
        let app = &self.app;
        let mut frame = Frame::new(bounds.size());

        for (_, router) in &app.devices {
            let position = Point {
                x: router.x,
                y: router.y,
            };
            let text = iced::canvas::Text {
                position,
                content: router.name.clone(),
                ..Default::default()
            };
            frame.fill_text(text);
            frame.fill_rectangle(
                position,
                Size {
                    width: 40.0,
                    height: 40.0,
                },
                Color::new(1., 0., 0., 1.),
            )
        }

        vec![frame.into_geometry()]
    }

    fn update(
        &mut self,
        ev: Event,
        bounds: Rectangle<f32>,
        cursor: Cursor,
    ) -> (Status, Option<Message>) {
        match ev {
            Event::Mouse(MouseEvt::ButtonPressed(MouseBtn::Left)) => {
                for (key, router) in &self.app.devices {
                    let router_area = Rectangle::new(
                        Point {
                            x: router.x,
                            y: router.y,
                        },
                        ROUTER_SIZE,
                    );

                    let msg = if let Some(pos) = cursor.position_in(&bounds) {
                        if router_area.contains(pos) {
                            Message::LeftMouseClicked(key)
                        } else {
                            Message::DeselectRouter
                        }
                    } else {
                        Message::DeselectRouter
                    };

                    return (Status::Captured, Some(msg));
                }

                (Status::Captured, None)
            }
            Event::Mouse(MouseEvt::ButtonReleased(MouseBtn::Left)) => {
                (Status::Captured, Some(Message::LeftMouseReleased))
            }
            Event::Mouse(MouseEvt::CursorMoved { mut position }) => {
                position.x -= bounds.x;
                position.y -= bounds.y;
                (Status::Captured, Some(Message::MouseMoved(position)))
            }
            Event::Mouse(MouseEvt::CursorEntered) => (Status::Ignored, None),
            Event::Mouse(MouseEvt::CursorLeft) => (Status::Ignored, None),
            Event::Keyboard(_) => (Status::Ignored, None),
            Event::Mouse(MouseEvt::WheelScrolled { .. }) => (Status::Ignored, None),
            Event::Mouse(MouseEvt::ButtonPressed(_) | MouseEvt::ButtonReleased(_)) => {
                (Status::Ignored, None)
            }
        }
    }
}
