let socket = null;

function connect() {
    let url = new URL("/ws/live", window.location.href);
    url.protocol = url.protocol.replace('http', 'ws');
    let socket = new WebSocket(url);

    socket.addEventListener('open', (event) => {
        var livebutton = document.getElementById("live-button");
        livebutton.checked = true;
    });

    socket.addEventListener('close', (event) => {
        var livebutton = document.getElementById("live-button");
        livebutton.checked = false;
    });

    socket.addEventListener('message', (event) => {
        let entry = JSON.parse(event.data);
        add_entry(entry);
    });

    return socket;
}

function add_entry(entry) {
    var table = document.getElementById("loglist");
    var row = table.insertRow(-1);
    row.insertCell(0).innerHTML = entry.timestamp;
    row.insertCell(1).innerHTML = entry.hostname;
    row.insertCell(2).innerHTML = entry.service;
    row.insertCell(3).innerHTML = entry.message;
}

window.addEventListener("load", () => {
    var livebutton = document.getElementById("live-button");
    livebutton.onchange = (e) => {
        if (livebutton.checked) {
            socket = connect();
        } else if (socket !== null) {
            socket.close();
            socket = null;
        }
    };
    socket = connect();
});