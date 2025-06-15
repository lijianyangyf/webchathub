// src/protocol.rs

use serde::{Deserialize, Serialize};

/// 客户端 → 服务器：请求
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum ClientRequest {
    /// 加入房间
    Join { room: String, name: String },

    /// 离开房间
    Leave { room: String },

    /// 发送聊天消息
    Message { room: String, text: String },

    /// 查询房间列表
    RoomList,

    /// 查询房间成员列表 (1-A)
    Members { room: String },
}

/// 服务器 → 客户端：事件推送
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum ServerEvent {
    /// 有用户加入
    UserJoined { room: String, name: String },

    /// 有用户离开
    UserLeft { room: String, name: String },

    /// 聊天消息；包含毫秒级时间戳
    NewMessage { room: String, name: String, text: String, ts: u64 },

    /// 当前房间列表
    RoomList { rooms: Vec<String> },

    /// 房间成员列表 (1-A)
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
