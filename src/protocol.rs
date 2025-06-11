// src/protocol.rs

use serde::{Serialize, Deserialize};

/// 客户端请求：加入房间、发送消息、离开房间
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum ClientRequest {
    Join  { room: String, name: String },
    Leave { room: String },
    Message { room: String, text: String },
    RoomList,
}

/// 服务端推送事件
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum ServerEvent {
    UserJoined { room: String, name: String },
    UserLeft   { room: String, name: String },
    NewMessage { room: String, name: String, text: String, ts: u64 },
    RoomList   { rooms: Vec<String> },
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_serialize_client_join() {
        let req = ClientRequest::Join {
            room: "rust".to_string(),
            name: "alice".to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("Join"));
        assert!(json.contains("rust"));
        assert!(json.contains("alice"));

        // 反序列化测试
        let back: ClientRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req, back);
    }

    #[test]
    fn test_serialize_server_new_message() {
        let event = ServerEvent::NewMessage {
            room: "rust".to_string(),
            name: "bob".to_string(),
            text: "hello!".to_string(),
            ts: 1234567890,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("NewMessage"));
        assert!(json.contains("bob"));
        assert!(json.contains("hello"));
        assert!(json.contains("1234567890"));

        // 反序列化测试
        let back: ServerEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);
    }

    #[test]
    fn test_room_list() {
        let event = ServerEvent::RoomList {
            rooms: vec!["room1".to_string(), "room2".to_string()],
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("RoomList"));
        assert!(json.contains("room1"));
        assert!(json.contains("room2"));

        // 反序列化测试
        let back: ServerEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);
    }
}
