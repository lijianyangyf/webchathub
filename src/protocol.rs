use serde::{Deserialize, Serialize};


#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum ClientRequest {

    Join { room: String, name: String },

    Leave { room: String },

    Message { room: String, text: String },

    RoomList,

    Members { room: String },
}


#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum ServerEvent {

    UserJoined { room: String, name: String },

    UserLeft { room: String, name: String },

    NewMessage { room: String, name: String, text: String, ts: u64 },

    RoomList { rooms: Vec<String> },

    MemberList { room: String, members: Vec<String> },
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn serialize_join() {
        let req = ClientRequest::Join {
            room: "rust".into(),
            name: "alice".into(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert_eq!(serde_json::from_str::<ClientRequest>(&json).unwrap(), req);
    }

    #[test]
    fn serialize_new_msg() {
        let ev = ServerEvent::NewMessage {
            room: "rust".into(),
            name: "bob".into(),
            text: "hello".into(),
            ts: 123,
        };
        let json = serde_json::to_string(&ev).unwrap();
        assert_eq!(serde_json::from_str::<ServerEvent>(&json).unwrap(), ev);
    }

    #[test]
    fn serialize_member_list() {
        let ev = ServerEvent::MemberList {
            room: "rust".into(),
            members: vec!["alice".into(), "bob".into()],
        };
        let json = serde_json::to_string(&ev).unwrap();
        assert_eq!(serde_json::from_str::<ServerEvent>(&json).unwrap(), ev);
    }
}
