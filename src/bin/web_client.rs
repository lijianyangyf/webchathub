//! web_client.rs – serve a one‑page HTML/JS client with a TUI‑like look (matching ui.rs)
use std::{env, net::SocketAddr};
use anyhow::Result;
use warp::Filter;

#[tokio::main]
async fn main() -> Result<()> {
    // Usage: web_client [ws://addr:port] [http_port]
    let mut args = env::args().skip(1);
    let ws_addr = args.next().unwrap_or_else(|| "ws://127.0.0.1:9000".into());
    let port: u16 = args.next().and_then(|p| p.parse().ok()).unwrap_or(8000);

    // Build static page once then serve it for every request
    let html = build_html(&ws_addr);
    let route = warp::path::end().map(move || warp::reply::html(html.clone()));

    let listen: SocketAddr = ([127, 0, 0, 1], port).into();
    let url = format!("http://{}/", listen);
    eprintln!("Serving chat UI at {}", url);
    let _ = webbrowser::open(&url);

    warp::serve(route).run(listen).await;
    Ok(())
}

/// Produce a self‑contained HTML page.  `WS_ADDR` placeholder is substituted.
fn build_html(ws_addr: &str) -> String {
    const PAGE: &str = r#"<!DOCTYPE html><html lang='en'>
<head><meta charset='utf-8'/>
<title>Rust Chat</title>
<style>
    :root {
        --bg: #1e1e1e;
        --fg: #f0f0f0;
        --accent: #555;
        --border: #555;
    }
    html,body{height:100%;margin:0;background:var(--bg);color:var(--fg);font-family:monospace;display:flex;flex-direction:column}

    /* Top bar */
    #top-bar{display:flex;justify-content:space-between;align-items:center;padding:4px 8px;background:#333;border-bottom:1px solid var(--border)}
    #top-bar h1{font-size:1rem;margin:0;color:var(--fg)}

    /* Buttons */
    button{background:var(--accent);color:var(--fg);border:none;padding:4px 12px;border-radius:4px;cursor:pointer}
    button:disabled{opacity:.4;cursor:not-allowed}

    /* Panels mimic tui::widgets::Block */
    .panel{position:relative;border:1px solid var(--border);border-radius:4px;margin:6px;display:flex;flex-direction:column}
    .panel::before{content:attr(data-title);position:absolute;top:-0.65em;left:8px;background:var(--bg);padding:0 4px;font-size:.8em;color:#999}

    #log{flex:1;overflow-y:auto;padding:8px;white-space:pre-wrap}
    #input-bar{flex:none;flex-direction:row;gap:4px;padding:4px;align-items:center}
    #input{flex:1;background:#222;color:var(--fg);border:none;outline:none;padding:4px;font-family:inherit}
</style></head><body>
    <div id='top-bar'>
        <h1>Rust Chat</h1>
        <div>
            <button id='btn-rooms'>房间列表</button>
            <button id='btn-leave' disabled>离开</button>
        </div>
    </div>

    <div id='log' class='panel' data-title='聊天记录'></div>

    <div id='input-bar' class='panel' data-title='输入'>
        <input id='input' placeholder='> '/>
        <button id='send'>发送</button>
    </div>

<script>
// Utility helpers ------------------------------------------------------------
const log = document.getElementById('log');
function append(m){const d=document.createElement('div');d.textContent=m;log.appendChild(d);log.scrollTop=log.scrollHeight;}
function pkt(variant,data){if(data===undefined)return JSON.stringify(variant);const o={};o[variant]=data;return JSON.stringify(o);}

// WebSocket ------------------------------------------------------------------
const ws = new WebSocket('WS_ADDR');
ws.addEventListener('open',  ()=>append('[connected]'));
ws.addEventListener('close', ()=>append('[connection closed]'));
ws.addEventListener('error', e=>append('[error] '+e.message));
ws.addEventListener('message', ev=>{
    // Attempt to parse server event JSON and pretty‑print
    const raw = ev.data;
    try {
        const obj = JSON.parse(raw);
        const [k,v] = Object.entries(obj)[0] || [];
        switch(k){
            case 'NewMessage':
            case 'Message':
                append(`${v.name} : ${v.text}`);
                return;
            case 'UserJoined':
                append(`👤 ${v.name} 加入了房间`);
                return;
            case 'UserLeft':
                append(`👋 ${v.name} 离开了房间`);
                return;
            case 'RoomList':
                append(`当前房间列表: ${JSON.stringify(v.rooms)}`);
                return;
        }
    } catch(e) { /* fall through */ }
    append(raw); // fallback to raw string
});

// State ----------------------------------------------------------------------
let currentRoom=null;const leaveBtn=document.getElementById('btn-leave');

// Sending logic (shared by Enter key & 发送 button) ---------------------------
function doSend(){
    const box=document.getElementById('input');
    const txt=box.value.trim(); if(!txt) return; box.value='';
    let msg;
    if(txt.startsWith('/rooms')){
        msg=pkt('RoomList');
    }else if(txt.startsWith('/join ')){
        const [,room,nick]=txt.split(' ');
        if(!room||!nick){append('[usage] /join <room> <nick>');return;}
        currentRoom=room; leaveBtn.disabled=false;
        msg=pkt('Join',{room,name:nick});
    }else if(txt.startsWith('/leave')){
        leave(); return; // will call ws.send inside leave()
    }else{
        if(!currentRoom){append('[join a room first]');return;}
        msg=pkt('Message',{room:currentRoom,text:txt});
    }
    ws.send(msg);
}

document.getElementById('send').addEventListener('click', doSend);
document.getElementById('input').addEventListener('keydown', e=>{if(e.key==='Enter') doSend();});

document.getElementById('btn-rooms').addEventListener('click', ()=>ws.send(pkt('RoomList')));

document.getElementById('btn-leave').addEventListener('click', leave);
function leave(){
    if(!currentRoom){append('[未加入房间]');return;}
    ws.send(pkt('Leave',{room:currentRoom}));
    append(`[离开房间] ${currentRoom}`);
    currentRoom=null; leaveBtn.disabled=true;
}
</script>
</body></html>"#;
    PAGE.replace("WS_ADDR", ws_addr)
}
