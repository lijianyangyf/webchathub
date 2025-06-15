// src/protocol.rs

use serde::{Serialize, Deserialize};

/// 客户端向服务器发送的请求。
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum ClientRequest {
    /// 加入指定房间并声明用户名。
    Join { room: String, name: String },
    /// 离开当前所在房间（按房间名）。
    Leave { room: String },
    /// 向房间广播一条文本消息。
    Message { room: String, text: String },
    /// 请求当前所有房间列表。
    RoomList,
    /// **新增**：查询指定房间的在线成员列表。
    Members { room: String },
}

/// 服务器推送给客户端的事件。
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum ServerEvent {
    /// 有新用户进入房间。
    UserJoined { room: String, name: String },
    /// 有用户离开房间。
    UserLeft { room: String, name: String },
    /// 新消息。
    NewMessage { room: String, name: String, text: String, ts: u64 },
    /// 当前房间列表。
    RoomList { rooms: Vec<String> },
    /// **新增**：返回房间成员列表。
    MemberList { room: String, members: Vec<String> },
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

    #[test]
    fn serialize_client_members() {
        let req = ClientRequest::Members {
            room: "rust".into(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("Members"));
        assert!(json.contains("rust"));

        let back: ClientRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req, back);
    }

    #[test]
    fn serialize_server_member_list() {
        let event = ServerEvent::MemberList {
            room: "rust".into(),
            members: vec!["alice".into(), "bob".into()],
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("MemberList"));
        assert!(json.contains("alice"));
        assert!(json.contains("bob"));

        let back: ServerEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);
    }
}
