let sockets = null;

const debounce = (callback, wait) => {
    let timeoutId = null;
    return (...args) => {
        window.clearTimeout(timeoutId);
        timeoutId = window.setTimeout(() => {
            callback.apply(null, args);
        }, wait);
    };
};

function get_hosts() {
    const params = new Proxy(new URLSearchParams(window.location.search), {
        get: (searchParams, prop) => searchParams.get(prop),
    });
    const hosts =  params.hosts.split(",");
    return hosts;
}

function build_url(hostname, ws) {
    let url
    hostname = hostname.replace(/\/+$/, "");
    if (ws == true) {
        url = new URL(hostname + "/ws/live", window.location.href);
        url.protocol = url.protocol.replace('http', 'ws');
    } else {
        url = new URL(hostname + "/api/extract", window.location.href);
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
    for (const host of get_hosts()) {
        let url = build_url(host, false);
        fetch(url)
            .then((response) => response.json())
            .then((entries) => entries.forEach(add_entry));
    }
}

function connect() {
    clear_table();
    populate_a_bit();

    let sockets = get_hosts().map(host => {
        let url = build_url(host, true);
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
            add_entry_and_scroll(entry);
        });

        return socket;
    });

    return sockets;
}

function add_entry_and_scroll(entry) {
    var autoscroll = window.innerHeight + window.scrollY >= document.body.offsetHeight;

    add_entry(entry);

    if (autoscroll) {
        window.scrollTo(0, document.body.scrollHeight);
    }
}

function add_entry(entry) {
    var table = document.getElementById("loglist-body");
    var row = table.insertRow(-1);
    row.insertCell(0).innerHTML = entry.timestamp;
    row.insertCell(1).innerHTML = entry.hostname;
    row.insertCell(2).innerHTML = entry.service;
    var message = document.createElement("pre");
    message.appendChild(document.createTextNode(entry.message));
    row.insertCell(3).appendChild(message);
}

window.addEventListener("load", () => {
    const hosts = get_hosts();
    var hoststext = document.getElementById("hosts");
    hoststext.value = hosts.join(",");

    var livebutton = document.getElementById("live-button");
    livebutton.onchange = (e) => {
        if (livebutton.checked) {
            sockets = connect();
        } else if (sockets !== null) {
            sockets.forEach(s => s.close());
            sockets = null;
        }
    };

    var services_filter = document.getElementById("services-filter");
    services_filter.oninput = debounce((e) => {
        if (sockets !== null) {
            sockets.forEach(s => s.close());
        }
        sockets = connect();
    }, 250);

    var message_keywords_filter = document.getElementById("message-keywords-filter");
    message_keywords_filter.oninput = debounce((e) => {
        if (sockets !== null) {
            sockets.forEach(s => s.close());
        }
        sockets = connect();
    }, 250);

    window.addEventListener("wheel", debounce((e) => {
        if (e.deltaY < 0 && window.scrollY == 0) {
            console.log("fetch some");
        }
    }, 200));

    sockets = connect();
});