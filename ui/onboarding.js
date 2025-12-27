// Tauri API
const { invoke } = window.__TAURI__.tauri;
const { listen } = window.__TAURI__.event;
const { open } = window.__TAURI__.shell;

// State
let currentStep = 1;
let permissions = {
    admin: false,
    accessibility: false,
    inputMonitoring: false,
    network: false
};
let discoveredDevices = [];
let pairedDevices = [];
let localDeviceInfo = null;

// Initialize
async function init() {
    console.log('Initializing onboarding...');
    
    // Detect platform
    await detectPlatform();
    
    // Set up event listeners
    setupEventListeners();
    
    // Check if onboarding was already completed
    const completed = await checkOnboardingCompleted();
    if (completed) {
        // Skip to main app
        window.location.href = 'index.html';
        return;
    }
    
    // Start with step 1
    showStep(1);
}

// Detect platform and show appropriate permissions
async function detectPlatform() {
    try {
        const platform = await invoke('get_platform');
        document.body.classList.add(`platform-${platform.toLowerCase()}`);
        console.log('Platform detected:', platform);
    } catch (error) {
        console.error('Failed to detect platform:', error);
        // Default to showing all permissions
    }
}

// Check if onboarding was completed
async function checkOnboardingCompleted() {
    try {
        const completed = await invoke('is_onboarding_completed');
        return completed;
    } catch (error) {
        console.error('Failed to check onboarding status:', error);
        return false;
    }
}

// Set up event listeners
function setupEventListeners() {
    // Step 1: Welcome
    document.getElementById('start-btn').addEventListener('click', () => {
        goToStep(2);
    });
    
    // Step 2: Permissions
    document.getElementById('back-to-welcome-btn').addEventListener('click', () => {
        goToStep(1);
    });
    
    document.getElementById('continue-to-pairing-btn').addEventListener('click', () => {
        goToStep(3);
    });
    
    // Permission request buttons
    const adminBtn = document.getElementById('request-admin-btn');
    if (adminBtn) {
        adminBtn.addEventListener('click', requestAdminPermission);
    }
    
    const accessibilityBtn = document.getElementById('request-accessibility-btn');
    if (accessibilityBtn) {
        accessibilityBtn.addEventListener('click', requestAccessibilityPermission);
    }
    
    const inputMonitoringBtn = document.getElementById('request-input-monitoring-btn');
    if (inputMonitoringBtn) {
        inputMonitoringBtn.addEventListener('click', requestInputMonitoringPermission);
    }
    
    // Step 3: Pairing
    document.getElementById('back-to-permissions-btn').addEventListener('click', () => {
        goToStep(2);
    });
    
    document.getElementById('skip-pairing-btn').addEventListener('click', () => {
        goToStep(4);
    });
    
    document.getElementById('continue-to-layout-btn').addEventListener('click', () => {
        goToStep(4);
    });
    
    // Step 4: Layout
    document.getElementById('back-to-pairing-btn').addEventListener('click', () => {
        goToStep(3);
    });
    
    document.getElementById('finish-btn').addEventListener('click', finishOnboarding);
}

// Navigate to a specific step
function goToStep(step) {
    // Hide current step
    document.querySelectorAll('.onboarding-step').forEach(el => {
        el.classList.remove('active');
    });
    
    // Update progress bar
    document.querySelectorAll('.progress-step').forEach((el, index) => {
        el.classList.remove('active', 'completed');
        if (index + 1 < step) {
            el.classList.add('completed');
        } else if (index + 1 === step) {
            el.classList.add('active');
        }
    });
    
    // Show new step
    document.getElementById(`step-${step}`).classList.add('active');
    currentStep = step;
    
    // Initialize step-specific logic
    if (step === 2) {
        initPermissionsStep();
    } else if (step === 3) {
        initPairingStep();
    } else if (step === 4) {
        initLayoutStep();
    }
}

// Show a specific step (alias for goToStep)
function showStep(step) {
    goToStep(step);
}

// Step 2: Permissions
async function initPermissionsStep() {
    console.log('Initializing permissions step...');
    await checkAllPermissions();
    
    // Start periodic permission checks
    const checkInterval = setInterval(async () => {
        if (currentStep !== 2) {
            clearInterval(checkInterval);
            return;
        }
        await checkAllPermissions();
    }, 2000);
}

async function checkAllPermissions() {
    try {
        const status = await invoke('check_permissions');
        console.log('Permission status:', status);
        
        // Update UI for each permission
        updatePermissionStatus('admin', status.admin);
        updatePermissionStatus('accessibility', status.accessibility);
        updatePermissionStatus('input-monitoring', status.input_monitoring);
        updatePermissionStatus('network', status.network);
        
        // Store in state
        permissions = {
            admin: status.admin,
            accessibility: status.accessibility,
            inputMonitoring: status.input_monitoring,
            network: status.network
        };
        
        // Check if we can continue
        const canContinue = checkCanContinueFromPermissions();
        document.getElementById('continue-to-pairing-btn').disabled = !canContinue;
        
        // Show firewall help if network permission is denied
        if (status.network === false) {
            const firewallHelp = document.getElementById('firewall-help');
            if (firewallHelp) {
                firewallHelp.style.display = 'block';
            }
        }
    } catch (error) {
        console.error('Failed to check permissions:', error);
    }
}

function updatePermissionStatus(permissionType, granted) {
    const statusEl = document.getElementById(`${permissionType}-status`);
    if (!statusEl) return;
    
    statusEl.className = 'permission-status';
    
    if (granted === true) {
        statusEl.textContent = 'Granted';
        statusEl.classList.add('granted');
        
        // Update parent permission item
        const permissionItem = statusEl.closest('.permission-item');
        if (permissionItem) {
            permissionItem.classList.add('granted');
        }
    } else if (granted === false) {
        statusEl.textContent = 'Denied';
        statusEl.classList.add('denied');
    } else {
        statusEl.textContent = 'Pending';
        statusEl.classList.add('pending');
    }
}

function checkCanContinueFromPermissions() {
    // On Windows, need admin permission
    // On macOS, need accessibility permission
    // Network permission is checked but not blocking
    
    const platform = document.body.className.includes('platform-windows') ? 'windows' : 'macos';
    
    if (platform === 'windows') {
        return permissions.admin === true;
    } else if (platform === 'macos') {
        return permissions.accessibility === true;
    }
    
    // If platform unknown, allow continue
    return true;
}

async function requestAdminPermission() {
    try {
        await invoke('request_admin_permission');
        await checkAllPermissions();
    } catch (error) {
        console.error('Failed to request admin permission:', error);
        alert('Failed to request admin permission: ' + error);
    }
}

async function requestAccessibilityPermission() {
    try {
        await invoke('open_accessibility_preferences');
    } catch (error) {
        console.error('Failed to open accessibility preferences:', error);
        alert('Failed to open System Preferences: ' + error);
    }
}

async function requestInputMonitoringPermission() {
    try {
        await invoke('open_input_monitoring_preferences');
    } catch (error) {
        console.error('Failed to open input monitoring preferences:', error);
        alert('Failed to open System Preferences: ' + error);
    }
}

// Step 3: Pairing
async function initPairingStep() {
    console.log('Initializing pairing step...');
    
    // Start device discovery
    await startDeviceDiscovery();
    
    // Listen for device events
    await listen('device-event', (event) => {
        handleDeviceEvent(event.payload);
    });
}

async function startDeviceDiscovery() {
    try {
        // Get initial devices
        discoveredDevices = await invoke('get_devices');
        renderDiscoveredDevices();
        
        // Refresh devices
        setTimeout(async () => {
            discoveredDevices = await invoke('refresh_devices');
            renderDiscoveredDevices();
        }, 1000);
    } catch (error) {
        console.error('Failed to discover devices:', error);
    }
}

function handleDeviceEvent(event) {
    if (currentStep !== 3) return;
    
    if (event.type === 'DeviceDiscovered') {
        const exists = discoveredDevices.find(d => d.id === event.device.id);
        if (!exists) {
            discoveredDevices.push(event.device);
            renderDiscoveredDevices();
        }
    } else if (event.type === 'DeviceLost') {
        discoveredDevices = discoveredDevices.filter(d => d.id !== event.device_id);
        renderDiscoveredDevices();
    } else if (event.type === 'DeviceConnected') {
        const device = discoveredDevices.find(d => d.id === event.device_id);
        if (device) {
            device.is_connected = true;
            if (!pairedDevices.find(d => d.id === device.id)) {
                pairedDevices.push(device);
            }
            renderDiscoveredDevices();
            updateContinueButton();
        }
    }
}

function renderDiscoveredDevices() {
    const container = document.getElementById('discovered-devices');
    
    if (discoveredDevices.length === 0) {
        container.innerHTML = '<p style="text-align: center; color: #6b7280;">No devices found yet. Make sure other devices are running KVM.</p>';
        return;
    }
    
    container.innerHTML = discoveredDevices.map(device => {
        const isPaired = device.is_connected || pairedDevices.find(d => d.id === device.id);
        const osIcon = device.os_type === 'Windows' ? '🪟' : '🍎';
        
        return `
            <div class="pairing-device-card ${isPaired ? 'paired' : ''}" data-device-id="${device.id}">
                <div class="pairing-device-header">
                    <span class="pairing-device-icon">${osIcon}</span>
                    <div>
                        <div class="pairing-device-name">${escapeHtml(device.name)}</div>
                        <div class="pairing-device-os">${escapeHtml(device.os_type)}</div>
                    </div>
                </div>
                <div class="pairing-device-status ${isPaired ? 'paired' : 'available'}">
                    ${isPaired ? '✓ Paired' : 'Click to Pair'}
                </div>
            </div>
        `;
    }).join('');
    
    // Add click handlers
    container.querySelectorAll('.pairing-device-card').forEach(card => {
        card.addEventListener('click', async () => {
            const deviceId = card.dataset.deviceId;
            await pairDevice(deviceId);
        });
    });
}

async function pairDevice(deviceId) {
    try {
        await invoke('connect_device', { deviceId });
        console.log('Paired with device:', deviceId);
    } catch (error) {
        console.error('Failed to pair device:', error);
        alert('Failed to pair device: ' + error);
    }
}

function updateContinueButton() {
    const continueBtn = document.getElementById('continue-to-layout-btn');
    continueBtn.disabled = pairedDevices.length === 0;
}

// Step 4: Layout
async function initLayoutStep() {
    console.log('Initializing layout step...');
    
    // Get local device info
    try {
        localDeviceInfo = await invoke('get_local_device_info');
        document.getElementById('local-device-name').textContent = localDeviceInfo.name;
    } catch (error) {
        console.error('Failed to get local device info:', error);
    }
    
    // Render paired devices on canvas
    renderLayoutDevices();
    
    // Set up drag and drop
    setupDragAndDrop();
}

function renderLayoutDevices() {
    const canvas = document.getElementById('tutorial-canvas');
    
    // Add paired devices to canvas
    pairedDevices.forEach((device, index) => {
        const deviceEl = document.createElement('div');
        deviceEl.className = 'tutorial-device';
        deviceEl.dataset.deviceId = device.id;
        deviceEl.innerHTML = `
            <div class="device-label">Remote Device</div>
            <div class="device-name">${escapeHtml(device.name)}</div>
        `;
        
        // Position devices in a circle around the local device
        const angle = (index / pairedDevices.length) * 2 * Math.PI;
        const radius = 150;
        const x = 50 + Math.cos(angle) * radius;
        const y = 50 + Math.sin(angle) * radius;
        
        deviceEl.style.left = `${x}%`;
        deviceEl.style.top = `${y}%`;
        deviceEl.style.transform = 'translate(-50%, -50%)';
        
        canvas.appendChild(deviceEl);
    });
}

function setupDragAndDrop() {
    const canvas = document.getElementById('tutorial-canvas');
    const devices = canvas.querySelectorAll('.tutorial-device:not(.local-device)');
    
    devices.forEach(device => {
        let isDragging = false;
        let startX, startY, initialLeft, initialTop;
        
        device.addEventListener('mousedown', (e) => {
            isDragging = true;
            startX = e.clientX;
            startY = e.clientY;
            
            const rect = device.getBoundingClientRect();
            const canvasRect = canvas.getBoundingClientRect();
            initialLeft = rect.left - canvasRect.left;
            initialTop = rect.top - canvasRect.top;
            
            device.style.cursor = 'grabbing';
        });
        
        document.addEventListener('mousemove', (e) => {
            if (!isDragging) return;
            
            const deltaX = e.clientX - startX;
            const deltaY = e.clientY - startY;
            
            const newLeft = initialLeft + deltaX;
            const newTop = initialTop + deltaY;
            
            device.style.left = `${newLeft}px`;
            device.style.top = `${newTop}px`;
            device.style.transform = 'none';
        });
        
        document.addEventListener('mouseup', () => {
            if (isDragging) {
                isDragging = false;
                device.style.cursor = 'move';
            }
        });
    });
}

// Finish onboarding
async function finishOnboarding() {
    try {
        // Save layout configuration
        await saveLayoutConfiguration();
        
        // Mark onboarding as completed
        await invoke('complete_onboarding');
        
        // Redirect to main app
        window.location.href = 'index.html';
    } catch (error) {
        console.error('Failed to finish onboarding:', error);
        alert('Failed to complete onboarding: ' + error);
    }
}

async function saveLayoutConfiguration() {
    const canvas = document.getElementById('tutorial-canvas');
    const devices = canvas.querySelectorAll('.tutorial-device:not(.local-device)');
    
    const layout = {};
    
    devices.forEach(device => {
        const deviceId = device.dataset.deviceId;
        const rect = device.getBoundingClientRect();
        const canvasRect = canvas.getBoundingClientRect();
        
        const x = Math.round(rect.left - canvasRect.left);
        const y = Math.round(rect.top - canvasRect.top);
        
        layout[deviceId] = {
            x,
            y,
            width: 1920, // Default width
            height: 1080, // Default height
            enabled_edges: {
                top: true,
                bottom: true,
                left: true,
                right: true
            }
        };
    });
    
    try {
        await invoke('save_layout_config', { 
            layout: { devices: layout }
        });
        console.log('Layout saved:', layout);
    } catch (error) {
        console.error('Failed to save layout:', error);
        throw error;
    }
}

// Utility function
function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}

// Start the app
init();
