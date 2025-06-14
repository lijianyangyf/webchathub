//! src/bin/web_client.rs
//! ----------------------------------------------------------
//! $ cargo run --bin web_client ws://127.0.0.1:9000 8000
//!  - 起一个本地 HTTP 服务 (端口可自选，默认 8000)
//!  - 自动打开浏览器
//!  - 支持房间列表 → 加入 → 聊天 → 离开（离开后关闭页面并结束进程）

use std::sync::{Arc, Mutex};

use tokio::sync::oneshot;
use warp::{Filter, Reply};

/*──────────────────── HTML 模板 ────────────────────*/
const HTML: &str = r#"
<!DOCTYPE html><html lang="zh-CN"><head><meta charset="utf-8"/>
<title>Rust Chat</title>
<style>
html,body{margin:0;height:100%;background:#1e1e1e;color:#ddd;font:14px/1.4 "JetBrains Mono",monospace}
#topbar{background:#2d2d2d;padding:6px 8px;display:flex;align-items:center;gap:8px}
#title{flex:1;font-weight:bold}
button{background:#5865f2;border:none;border-radius:4px;padding:4px 10px;color:#fff;cursor:pointer}
button:disabled{opacity:.35;cursor:default}
#messages{height:calc(100vh - 150px);overflow-y:auto;padding:8px;border:1px solid #555;margin:8px;border-radius:4px;white-space:pre-wrap}
#inputbar{display:flex;gap:6px;padding:0 8px 8px}
#input{flex:1;padding:6px;border:1px solid #555;border-radius:4px;background:#2d2d2d;color:#ddd}
#joinModal{display:none;position:fixed;inset:0;background:#0008;align-items:center;justify-content:center}
#joinCard{background:#2d2d2d;padding:16px 20px;border-radius:6px;display:flex;flex-direction:column;gap:10px;width:260px}
label{display:flex;flex-direction:column;gap:2px;font-size:12px}
input[type=text]{padding:6px;border:1px solid #555;border-radius:4px;background:#1e1e1e;color:#ddd}
</style></head><body>
<header id="topbar">
  <div id="title">Not joined</div>
  <button id="roomsBtn">房间列表</button>
  <button id="joinBtn">加入房间</button>
  <button id="leaveBtn" disabled>离开房间</button>
</header>
<main id="messages"></main>
<footer id="inputbar">
  <input id="input" placeholder="输入消息，Enter 发送" autocomplete="off"/>
  <button id="sendBtn">发送</button>
</footer>

<!-- —— Join modal —— -->
<div id="joinModal">
  <div id="joinCard">
    <label>房间<input id="roomFld" type="text" autocomplete="off"/></label>
    <label>昵称<input id="nickFld" type="text" autocomplete="off"/></label>
    <button id="joinOk">进入</button>
  </div>
</div>

<script>
(() => {
  const WS_URL = "%WS%";

  let joined = false,
      currentRoom = "",
      currentNick = "",
      pendingRoom = "";     // 发送 Join 后等待服务器确认

  /* ---------- DOM ---------- */
  const $ = id => document.getElementById(id);
  const msgBox   = $("messages");
  const input    = $("input");
  const sendBtn  = $("sendBtn");
  const roomsBtn = $("roomsBtn");
  const joinBtn  = $("joinBtn");
  const leaveBtn = $("leaveBtn");
  const joinModal= $("joinModal");
  const roomFld  = $("roomFld");
  const nickFld  = $("nickFld");
  const joinOk   = $("joinOk");
  const titleBar = $("title");

  const println = t => { msgBox.textContent += t + "\n"; msgBox.scrollTop = msgBox.scrollHeight; };

  /* ---------- 序列化成服务器理解的 JSON ---------- */
  function pkt(tag, data){
    return data === undefined
      ? JSON.stringify(tag)            // unit variant
      : JSON.stringify({ [tag]: data }); // struct variant
  }

  /* ---------- WebSocket ---------- */
  const ws = new WebSocket(WS_URL);
  ws.onopen  = () => println("[已连接到服务器]");
  ws.onerror = e  => println("[WS 错误] "+e);
  ws.onclose = () => println("[连接已断开]");

  ws.onmessage = ev => {
    let v;
    try { v = JSON.parse(ev.data); }
    catch { maybePlainJoined(ev.data); return; }

    if (typeof v === "string") { maybePlainJoined(v); return; }

    const tag = Object.keys(v)[0];
    const d   = v[tag];

    switch(tag){
      case "RoomList":
        println("[房间列表] "+ d.rooms.join(", "));
        break;

      /* ----------- 自己加入成功的多种可能变体 ----------- */
      case "Joined":
      case "JoinAck":
      case "JoinedRoom":
        mark_joined(d.room, d.name ?? currentNick);
        break;

      case "UserJoined":       /* 服务器广播，包括自己 */
        println(`👤 ${d.name} 加入了房间`);
        if (!joined && d.name === currentNick)       // 这就是我自己
          mark_joined(d.room ?? pendingRoom, d.name);
        break;

      /* ----------- 聊天 / 离开 ----------- */
      case "NewMessage":
        println(`${d.name} : ${d.text ?? d.msg}`);
        break;

      case "UserLeft":
        println(`👋 ${d.name} 离开了房间`);
        break;

      case "Left":
        local_leave();
        break;

      default:
        println(ev.data);
    }
  };

  /* ---------- helpers ---------- */
  function mark_joined(room, nick){
    joined = true;
    currentRoom = room;
    currentNick = nick;
    titleBar.textContent = `${nick}@${room}`;
    joinBtn.disabled  = true;
    leaveBtn.disabled = false;
    println(`[已加入] ${room}`);
  }

  function local_leave(){
    joined = false; currentRoom = "";
    titleBar.textContent = "Not joined";
    joinBtn.disabled = false; leaveBtn.disabled = true;
    fetch("/shutdown",{method:"POST"}).finally(()=>{
      try{window.close();}catch{}
      window.location.replace("about:blank");
    });
  }

  /** 服务器可能发送纯文本 "Joined room xxx" */
  function maybePlainJoined(s){
    const m = /^Joined\b.*?(?=room\s+)?\s+([^\s]+)$/i.exec(s);
    if (m && !joined) mark_joined(m[1], currentNick);
    else println(s);
  }

  /* ---------- UI events ---------- */
  roomsBtn.onclick = ()=>ws.send(pkt("RoomList"));

  joinBtn.onclick  = ()=>{
    joinModal.style.display="flex";
    roomFld.value = nickFld.value = "";
    roomFld.focus();
  };

  joinOk.onclick   = ()=>{
    const r = roomFld.value.trim(), n = nickFld.value.trim();
    if(!r||!n) return;
    pendingRoom = r; currentNick = n;
    ws.send(pkt("Join",{ room:r, name:n }));
    joinModal.style.display="none";
  };

  leaveBtn.onclick = ()=>{ if(joined) ws.send(pkt("Leave")); };

  sendBtn.onclick  = ()=>{
    const txt = input.value.trim();
    if(!txt||!joined) return;
    ws.send(pkt("Message",{ room:currentRoom, text:txt }));  /* 如需 msg:txt 请改字段 */
    input.value="";
  };

  /* Enter 键快捷 */
  input.addEventListener("keypress",e=>{ if(e.key==="Enter") sendBtn.onclick(); });
  roomFld.addEventListener("keypress",e=>{ if(e.key==="Enter") joinOk.onclick(); });
  nickFld.addEventListener("keypress",e=>{ if(e.key==="Enter") joinOk.onclick(); });
})();
</script></body></html>
"#;

/*──────────────────── warp 过滤器 ────────────────────*/
fn routes(
    html: String,
    tx: Arc<Mutex<Option<oneshot::Sender<()>>>>,
) -> impl Filter<Extract = impl Reply, Error = warp::Rejection> + Clone {
    let page = warp::path::end().map(move || warp::reply::html(html.clone()));

    let shutdown = warp::path("shutdown")
        .and(warp::post())
        .map(move || {
            if let Some(s) = tx.lock().unwrap().take() {
                let _ = s.send(());
            }
            "ok"
        });

    page.or(shutdown)
}

/*──────────────────── main ───────────────────────────*/
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    /* ---- CLI (<ws-url> <port>)，留默认方便双击 ---- */
    let mut args = std::env::args().skip(1);
    let ws_url   = args.next().unwrap_or_else(|| "ws://127.0.0.1:9000".into());
    let port: u16 = args.next()
        .unwrap_or_else(|| "8000".into())
        .parse()
        .expect("bad port");

    /* ---- build HTML + shutdown channel ---- */
    let html = HTML.replace("%WS%", &ws_url);
    let (tx, rx) = oneshot::channel::<()>();
    let tx = Arc::new(Mutex::new(Some(tx)));

    /* ---- HTTP server ---- */
    let (addr, server) = warp::serve(routes(html, tx))
        .bind_ephemeral(([127, 0, 0, 1], port));
    tokio::spawn(server);

    /* ---- auto-open browser ---- */
    if let Err(e) = open::that(format!("http://{}", addr)) {
        eprintln!("Failed to open browser: {e}");
    }

    /* ---- wait for /shutdown ---- */
    let _ = rx.await;
    println!("Shutdown signal received – exiting.");
    Ok(())
}
