use super::model::AppModel;
use super::message::Message;

pub fn update(model: &mut AppModel, msg: Message) {
    match msg {
        Message::Quit => {
            model.running = false;
        }
        Message::Tick => {
            // handle tick
        }
    }
}
