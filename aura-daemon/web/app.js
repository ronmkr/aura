let ws;
let rpcId = 1;
let tasks = {};

const elements = {
    tokenInput: document.getElementById('token-input'),
    connectBtn: document.getElementById('connect-btn'),
    statusIndicator: document.getElementById('connection-status'),
    uriInput: document.getElementById('uri-input'),
    addTaskBtn: document.getElementById('add-task-btn'),
    tasksContainer: document.getElementById('tasks-container'),
    globalDl: document.getElementById('global-dl')
};

function formatBytes(bytes) {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
}

async function rpcCall(method, params = []) {
    const token = elements.tokenInput.value;
    const headers = { 'Content-Type': 'application/json' };
    if (token) headers['X-Aura-Token'] = token;

    const body = {
        jsonrpc: '2.0',
        id: rpcId++,
        method,
        params
    };

    try {
        const res = await fetch('/jsonrpc', {
            method: 'POST',
            headers,
            body: JSON.stringify(body)
        });
        const data = await res.json();
        if (data.error) throw new Error(data.error.message || JSON.stringify(data.error));
        return data.result;
    } catch (e) {
        console.error('RPC Error:', e);
        alert(`RPC Error: ${e.message}`);
        throw e;
    }
}

function connectWebSocket() {
    if (ws) ws.close();
    
    const token = elements.tokenInput.value;
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    let url = `${protocol}//${window.location.host}/ws`;
    if (token) {
        url += `?token=${encodeURIComponent(token)}`;
    }

    ws = new WebSocket(url);

    ws.onopen = () => {
        elements.statusIndicator.className = 'status-indicator connected';
        refreshTasks(); // Fetch initial state
    };

    ws.onclose = () => {
        elements.statusIndicator.className = 'status-indicator disconnected';
        setTimeout(connectWebSocket, 5000); // Reconnect loop
    };

    ws.onmessage = (event) => {
        try {
            const data = JSON.parse(event.data);
            if (data.method === 'aura.onEvent') {
                handleAuraEvent(data.params);
            }
        } catch (e) {
            console.error('Failed to parse WS message', e);
        }
    };
}

function handleAuraEvent(event) {
    // Determine the type of event and handle accordingly
    if (event.TaskAdded) {
        // Just refresh the whole list for simplicity
        refreshTasks();
    } else if (event.TaskProgress) {
        const [id, downloaded, total, speed] = event.TaskProgress;
        updateTaskProgress(id, downloaded, total, speed);
    } else if (event.TaskCompleted) {
        const id = event.TaskCompleted;
        updateTaskStatus(id, 'Completed');
        setTimeout(refreshTasks, 2000);
    } else if (event.TaskError) {
        const [id, err] = event.TaskError;
        updateTaskStatus(id, `Error: ${err}`);
    } else if (event.TaskPaused) {
        const id = event.TaskPaused;
        updateTaskStatus(id, 'Paused');
    } else if (event.TaskResumed) {
        const id = event.TaskResumed;
        updateTaskStatus(id, 'Downloading');
    }
}

async function refreshTasks() {
    try {
        const activeTasks = await rpcCall('aura.tellActive');
        tasks = {};
        elements.tasksContainer.innerHTML = '';
        
        activeTasks.forEach(task => {
            tasks[task.gid] = task;
            renderTask(task);
        });
    } catch (e) {
        // Handled in rpcCall
    }
}

function renderTask(task) {
    const card = document.createElement('div');
    card.className = 'task-card';
    card.id = `task-${task.gid}`;
    
    const percentage = task.totalLength > 0 ? (task.completedLength / task.totalLength) * 100 : 0;
    
    card.innerHTML = `
        <div class="task-header">
            <span class="task-name">${task.name || 'Unknown'}</span>
            <span class="task-status" id="status-${task.gid}">${task.status.toUpperCase()}</span>
        </div>
        <div class="progress-bar-container">
            <div class="progress-bar" id="bar-${task.gid}" style="width: ${percentage}%"></div>
        </div>
        <div class="task-details">
            <span id="progress-${task.gid}">${formatBytes(task.completedLength)} / ${formatBytes(task.totalLength)}</span>
            <span id="speed-${task.gid}">0 B/s</span>
        </div>
        <div class="task-controls">
            ${task.status === 'paused' ? 
                `<button onclick="unpauseTask('${task.gid}')">Resume</button>` : 
                `<button class="warning" onclick="pauseTask('${task.gid}')">Pause</button>`
            }
            <button class="danger" onclick="removeTask('${task.gid}')">Remove</button>
        </div>
    `;
    elements.tasksContainer.appendChild(card);
}

function updateTaskProgress(gid, downloaded, total, speed) {
    const bar = document.getElementById(`bar-${gid}`);
    const progressText = document.getElementById(`progress-${gid}`);
    const speedText = document.getElementById(`speed-${gid}`);
    
    if (bar && progressText && speedText) {
        const percentage = total > 0 ? (downloaded / total) * 100 : 0;
        bar.style.width = `${percentage}%`;
        progressText.innerText = `${formatBytes(downloaded)} / ${formatBytes(total)}`;
        speedText.innerText = `${formatBytes(speed)}/s`;
    }
}

function updateTaskStatus(gid, status) {
    const statusEl = document.getElementById(`status-${gid}`);
    if (statusEl) {
        statusEl.innerText = status.toUpperCase();
    }
}

window.pauseTask = async (gid) => {
    await rpcCall('aura.pause', [[gid]]);
    refreshTasks();
};

window.unpauseTask = async (gid) => {
    await rpcCall('aura.unpause', [[gid]]);
    refreshTasks();
};

window.removeTask = async (gid) => {
    await rpcCall('aura.remove', [[gid]]);
    refreshTasks();
};

elements.connectBtn.onclick = () => {
    connectWebSocket();
    refreshTasks();
};

elements.addTaskBtn.onclick = async () => {
    const uri = elements.uriInput.value.trim();
    if (!uri) return;
    
    try {
        await rpcCall('aura.addUri', [[uri]]);
        elements.uriInput.value = '';
        setTimeout(refreshTasks, 500); // Give it a moment to appear
    } catch(e) {}
};

// Auto-connect on load
connectWebSocket();
