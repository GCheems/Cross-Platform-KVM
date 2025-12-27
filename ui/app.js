// Tauri API
const { invoke } = window.__TAURI__.tauri;
const { listen } = window.__TAURI__.event;

// State
let devices = [];
let isLoading = false;

// DOM Elements
const deviceList = document.getElementById('device-list');
const emptyState = document.getElementById('empty-state');
const refreshBtn = document.getElementById('refresh-btn');
const layoutBtn = document.getElementById('layout-btn');
const hotkeyBtn = document.getElementById('hotkey-btn');
const updateBtn = document.getElementById('update-btn');
const retryBtn = document.getElementById('retry-btn');
const statusDot = document.getElementById('status-dot');
const statusText = document.getElementById('status-text');

// Initialize
async function init() {
    console.log('Initializing KVM UI...');
    
    // Check if onboarding is completed
    try {
        const onboardingCompleted = await invoke('is_onboarding_completed');
        if (!onboardingCompleted) {
            // Redirect to onboarding
            window.location.href = 'onboarding.html';
            return;
        }
    } catch (error) {
        console.error('Failed to check onboarding status:', error);
        // Continue anyway
    }
    
    // Set up event listeners
    refreshBtn.addEventListener('click', refreshDevices);
    layoutBtn.addEventListener('click', () => window.location.href = 'layout.html');
    hotkeyBtn.addEventListener('click', () => window.location.href = 'hotkey.html');
    updateBtn.addEventListener('click', () => window.location.href = 'update.html');
    retryBtn.addEventListener('click', refreshDevices);
    
    // Listen for device events from backend
    await setupEventListeners();
    
    // Load initial devices
    await loadDevices();
}

// Set up event listeners for real-time updates
async function setupEventListeners() {
    try {
        // Tell backend to start sending events
        await invoke('listen_device_events');
        
        // Listen for device events
        await listen('device-event', (event) => {
            console.log('Device event:', event.payload);
            handleDeviceEvent(event.payload);
        });
        
        console.log('Event listeners set up successfully');
    } catch (error) {
        console.error('Failed to set up event listeners:', error);
    }
}

// Handle device events from backend
function handleDeviceEvent(event) {
    switch (event.type) {
        case 'DeviceDiscovered':
            addOrUpdateDevice(event.device);
            updateStatus('online', `${devices.length} device(s) found`);
            notificationManager.success(
                'Device Discovered',
                `Found ${event.device.name} (${event.device.os_type})`,
                3000
            );
            break;
        case 'DeviceLost':
            const lostDevice = devices.find(d => d.id === event.device_id);
            removeDevice(event.device_id);
            updateStatus('online', `${devices.length} device(s) found`);
            if (lostDevice) {
                notificationManager.warning(
                    'Device Lost',
                    `${lostDevice.name} is no longer available`,
                    3000
                );
            }
            break;
        case 'DeviceConnected':
            updateDeviceStatus(event.device_id, 'Connected', true);
            const connectedDevice = devices.find(d => d.id === event.device_id);
            if (connectedDevice) {
                notificationManager.success(
                    'Device Connected',
                    `Successfully connected to ${connectedDevice.name}`,
                    3000
                );
            }
            break;
        case 'DeviceDisconnected':
            updateDeviceStatus(event.device_id, 'Online', false);
            const disconnectedDevice = devices.find(d => d.id === event.device_id);
            if (disconnectedDevice) {
                notificationManager.info(
                    'Device Disconnected',
                    `Disconnected from ${disconnectedDevice.name}`,
                    3000
                );
            }
            break;
        case 'StatusUpdate':
            updateDeviceStatus(event.device_id, event.status, null);
            break;
        case 'DeviceSwitched':
            const switchedDevice = devices.find(d => d.id === event.device_id);
            if (switchedDevice) {
                notificationManager.success(
                    'Device Switched',
                    `Now controlling ${event.device_name}`,
                    2000
                );
                activeDeviceIndicator.show(event.device_name, 3000);
            }
            break;
        case 'ConnectionError':
            const errorDevice = devices.find(d => d.id === event.device_id);
            const deviceName = errorDevice ? errorDevice.name : 'Unknown Device';
            notificationManager.error(
                'Connection Error',
                `Failed to connect to ${deviceName}: ${event.error}`
            );
            break;
        case 'NetworkWarning':
            notificationManager.warning(
                'Network Warning',
                event.message,
                5000
            );
            break;
        case 'EdgeHighlight':
            edgeHighlightManager.show(event.edge, 500);
            break;
    }
}

// Add or update a device in the list
function addOrUpdateDevice(device) {
    const index = devices.findIndex(d => d.id === device.id);
    if (index >= 0) {
        devices[index] = device;
    } else {
        devices.push(device);
    }
    renderDevices();
}

// Remove a device from the list
function removeDevice(deviceId) {
    devices = devices.filter(d => d.id !== deviceId);
    renderDevices();
}

// Update device status
function updateDeviceStatus(deviceId, status, isConnected) {
    const device = devices.find(d => d.id === deviceId);
    if (device) {
        device.status = status;
        if (isConnected !== null) {
            device.is_connected = isConnected;
        }
        renderDevices();
    }
}

// Load devices from backend
async function loadDevices() {
    if (isLoading) return;
    
    isLoading = true;
    showLoading();
    updateStatus('scanning', 'Scanning for devices...');
    
    try {
        devices = await invoke('get_devices');
        console.log('Loaded devices:', devices);
        renderDevices();
        updateStatus('online', `${devices.length} device(s) found`);
    } catch (error) {
        console.error('Failed to load devices:', error);
        showError('Failed to load devices: ' + error);
        updateStatus('error', 'Error loading devices');
    } finally {
        isLoading = false;
    }
}

// Refresh device list
async function refreshDevices() {
    if (isLoading) return;
    
    isLoading = true;
    showLoading();
    updateStatus('scanning', 'Refreshing...');
    
    try {
        devices = await invoke('refresh_devices');
        console.log('Refreshed devices:', devices);
        renderDevices();
        updateStatus('online', `${devices.length} device(s) found`);
    } catch (error) {
        console.error('Failed to refresh devices:', error);
        showError('Failed to refresh devices: ' + error);
        updateStatus('error', 'Error refreshing devices');
    } finally {
        isLoading = false;
    }
}

// Connect to a device
async function connectDevice(deviceId) {
    try {
        await invoke('connect_device', { deviceId });
        console.log('Connected to device:', deviceId);
    } catch (error) {
        console.error('Failed to connect to device:', error);
        alert('Failed to connect: ' + error);
    }
}

// Disconnect from a device
async function disconnectDevice(deviceId) {
    try {
        await invoke('disconnect_device', { deviceId });
        console.log('Disconnected from device:', deviceId);
    } catch (error) {
        console.error('Failed to disconnect from device:', error);
        alert('Failed to disconnect: ' + error);
    }
}

// Render devices
function renderDevices() {
    if (devices.length === 0) {
        deviceList.style.display = 'none';
        emptyState.style.display = 'block';
        return;
    }
    
    deviceList.style.display = 'block';
    emptyState.style.display = 'none';
    
    deviceList.innerHTML = devices.map(device => createDeviceCard(device)).join('');
    
    // Add event listeners to buttons
    devices.forEach(device => {
        const connectBtn = document.getElementById(`connect-${device.id}`);
        const disconnectBtn = document.getElementById(`disconnect-${device.id}`);
        
        if (connectBtn) {
            connectBtn.addEventListener('click', () => connectDevice(device.id));
        }
        if (disconnectBtn) {
            disconnectBtn.addEventListener('click', () => disconnectDevice(device.id));
        }
    });
}

// Create device card HTML
function createDeviceCard(device) {
    const isConnected = device.is_connected;
    const statusClass = isConnected ? 'connected' : 'online';
    const statusText = device.status;
    
    return `
        <div class="device-card ${isConnected ? 'connected' : ''} fade-in">
            <div class="device-header">
                <div class="device-info">
                    <div class="device-name">${escapeHtml(device.name)}</div>
                    <div class="device-id">${escapeHtml(device.id)}</div>
                </div>
                <div class="device-status ${statusClass}">
                    <span>●</span>
                    ${statusText}
                </div>
            </div>
            
            <div class="device-details">
                <div class="detail-item">
                    <div class="detail-label">IP Address</div>
                    <div class="detail-value">${escapeHtml(device.ip_address)}</div>
                </div>
                <div class="detail-item">
                    <div class="detail-label">Operating System</div>
                    <div class="detail-value">${escapeHtml(device.os_type)}</div>
                </div>
            </div>
            
            <div class="device-actions">
                ${isConnected ? `
                    <button id="disconnect-${device.id}" class="btn btn-danger">
                        <span class="icon">🔌</span>
                        Disconnect
                    </button>
                ` : `
                    <button id="connect-${device.id}" class="btn btn-success">
                        <span class="icon">🔗</span>
                        Connect
                    </button>
                `}
            </div>
        </div>
    `;
}

// Show loading state
function showLoading() {
    deviceList.innerHTML = `
        <div class="loading">
            <div class="spinner"></div>
            <p>Discovering devices...</p>
        </div>
    `;
    deviceList.style.display = 'block';
    emptyState.style.display = 'none';
}

// Show error
function showError(message) {
    deviceList.innerHTML = `
        <div class="loading">
            <p style="color: #ef4444;">⚠️ ${escapeHtml(message)}</p>
        </div>
    `;
}

// Update status indicator
function updateStatus(type, text) {
    statusText.textContent = text;
    
    if (type === 'scanning') {
        statusDot.style.background = '#f59e0b';
    } else if (type === 'online') {
        statusDot.style.background = '#10b981';
    } else if (type === 'error') {
        statusDot.style.background = '#ef4444';
    }
}

// Escape HTML to prevent XSS
function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}

// Start the app
init();
