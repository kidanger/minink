let socket = null;

const debounce = (callback, wait) => {
    let timeoutId = null;
    return (...args) => {
        window.clearTimeout(timeoutId);
        timeoutId = window.setTimeout(() => {
            callback.apply(null, args);
        }, wait);
    };
};

function build_url(ws) {
    let url
    if (ws == true) {
        url = new URL("/ws/live", window.location.href);
        url.protocol = url.protocol.replace('http', 'ws');
    } else {
        url = new URL("/api/extract", window.location.href);
    }

    var services = document.getElementById("services-filter").value;
    if (services) {
        url.searchParams.append("services", services);
    }

    var message_keywords = document.getElementById("message-keywords-filter").value;
    if (message_keywords) {
        url.searchParams.append("message_keywords", message_keywords);
    }

    return url;
}

function clear_table() {
    var table = document.getElementById("loglist");
    var table_body = document.getElementById("loglist-body");
    var new_tbody = document.createElement('tbody');
    new_tbody.id = "loglist-body";
    table_body.parentNode.replaceChild(new_tbody, table_body);
}

function populate_a_bit() {
    let url = build_url(false);
    fetch(url)
        .then((response) => response.json())
        .then((entries) => entries.forEach(add_entry));
}

function connect() {
    clear_table();
    populate_a_bit();

    let url = build_url(true);
    console.dir(url);
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
    var table = document.getElementById("loglist-body");
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

    var services_filter = document.getElementById("services-filter");
    services_filter.oninput = debounce((e) => {
        if (socket !== null) {
            socket.close();
        }
        socket = connect();
    }, 250);

    var message_keywords_filter = document.getElementById("message-keywords-filter");
    message_keywords_filter.oninput = debounce((e) => {
        if (socket !== null) {
            socket.close();
        }
        socket = connect();
    }, 250);

    socket = connect();
});