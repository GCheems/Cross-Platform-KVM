// Tauri API
const { invoke } = window.__TAURI__.tauri;

// State
let devices = [];
let layoutConfig = { devices: {} };
let selectedDeviceId = null;
let draggedDevice = null;
let isDragging = false;

// Constants
const DEVICE_SCALE = 0.1; // Scale factor for display (1920px -> 192px)
const GRID_SIZE = 20;

// DOM Elements
const devicePalette = document.getElementById('device-palette');
const devicesContainer = document.getElementById('devices-container');
const canvas = document.getElementById('canvas');
const propertiesPanel = document.getElementById('properties-panel');
const backBtn = document.getElementById('back-btn');
const saveBtn = document.getElementById('save-btn');
const resetBtn = document.getElementById('reset-btn');
const removeDeviceBtn = document.getElementById('remove-device-btn');

// Edge switching checkboxes
const edgeLeft = document.getElementById('edge-left');
const edgeRight = document.getElementById('edge-right');
const edgeTop = document.getElementById('edge-top');
const edgeBottom = document.getElementById('edge-bottom');

// Position displays
const posX = document.getElementById('pos-x');
const posY = document.getElementById('pos-y');
const posWidth = document.getElementById('pos-width');
const posHeight = document.getElementById('pos-height');
const selectedDeviceName = document.getElementById('selected-device-name');

// Initialize
async function init() {
    console.log('Initializing Layout Editor...');
    
    // Set up event listeners
    backBtn.addEventListener('click', goBack);
    saveBtn.addEventListener('click', saveLayout);
    resetBtn.addEventListener('click', resetLayout);
    removeDeviceBtn.addEventListener('click', removeSelectedDevice);
    
    // Edge switching listeners
    edgeLeft.addEventListener('change', updateEdgeSwitching);
    edgeRight.addEventListener('change', updateEdgeSwitching);
    edgeTop.addEventListener('change', updateEdgeSwitching);
    edgeBottom.addEventListener('change', updateEdgeSwitching);
    
    // Canvas click to deselect
    canvas.addEventListener('click', (e) => {
        if (e.target === canvas || e.target.classList.contains('canvas-grid')) {
            deselectDevice();
        }
    });
    
    // Load devices and layout
    await loadDevices();
    await loadLayout();
}

// Load devices from backend
async function loadDevices() {
    try {
        devices = await invoke('get_devices');
        console.log('Loaded devices:', devices);
        renderDevicePalette();
    } catch (error) {
        console.error('Failed to load devices:', error);
        showError('Failed to load devices: ' + error);
    }
}

// Load layout configuration
async function loadLayout() {
    try {
        layoutConfig = await invoke('get_layout_config');
        console.log('Loaded layout:', layoutConfig);
        renderLayout();
    } catch (error) {
        console.error('Failed to load layout:', error);
        // Start with empty layout if none exists
        layoutConfig = { devices: {} };
        renderLayout();
    }
}

// Render device palette
function renderDevicePalette() {
    if (devices.length === 0) {
        devicePalette.innerHTML = '<p style="color: #6b7280; font-size: 13px;">No devices available</p>';
        return;
    }
    
    devicePalette.innerHTML = devices.map(device => {
        const inLayout = layoutConfig.devices[device.id] !== undefined;
        return `
            <div class="palette-device ${inLayout ? 'in-layout' : ''}" 
                 data-device-id="${device.id}"
                 draggable="${!inLayout}">
                <div class="palette-device-name">${escapeHtml(device.name)}</div>
                <div class="palette-device-info">${escapeHtml(device.os_type)}</div>
            </div>
        `;
    }).join('');
    
    // Add drag event listeners
    document.querySelectorAll('.palette-device').forEach(el => {
        if (!el.classList.contains('in-layout')) {
            el.addEventListener('dragstart', handlePaletteDragStart);
            el.addEventListener('dragend', handlePaletteDragEnd);
        }
    });
}

// Render layout on canvas
function renderLayout() {
    devicesContainer.innerHTML = '';
    
    for (const [deviceId, position] of Object.entries(layoutConfig.devices)) {
        const device = devices.find(d => d.id === deviceId);
        if (!device) continue;
        
        createCanvasDevice(device, position);
    }
}

// Create a device element on the canvas
function createCanvasDevice(device, position) {
    const deviceEl = document.createElement('div');
    deviceEl.className = 'canvas-device';
    deviceEl.dataset.deviceId = device.id;
    
    // Scale dimensions for display
    const displayWidth = position.width * DEVICE_SCALE;
    const displayHeight = position.height * DEVICE_SCALE;
    const displayX = position.x * DEVICE_SCALE;
    const displayY = position.y * DEVICE_SCALE;
    
    deviceEl.style.left = displayX + 'px';
    deviceEl.style.top = displayY + 'px';
    deviceEl.style.width = displayWidth + 'px';
    deviceEl.style.height = displayHeight + 'px';
    
    const icon = device.os_type === 'Windows' ? '🪟' : '🍎';
    
    deviceEl.innerHTML = `
        <div class="canvas-device-header">
            <div class="canvas-device-name">${escapeHtml(device.name)}</div>
            <div class="canvas-device-icon">${icon}</div>
        </div>
        <div class="canvas-device-info">${escapeHtml(device.os_type)}</div>
        <div class="canvas-device-resolution">${position.width} × ${position.height}</div>
    `;
    
    // Add edge indicators
    addEdgeIndicators(deviceEl, position.edge_switching || getDefaultEdgeSwitching());
    
    // Make draggable
    deviceEl.draggable = true;
    deviceEl.addEventListener('dragstart', handleCanvasDragStart);
    deviceEl.addEventListener('dragend', handleCanvasDragEnd);
    deviceEl.addEventListener('click', (e) => {
        e.stopPropagation();
        selectDevice(device.id);
    });
    
    devicesContainer.appendChild(deviceEl);
}

// Add edge indicators to device
function addEdgeIndicators(deviceEl, edgeSwitching) {
    const edges = ['left', 'right', 'top', 'bottom'];
    edges.forEach(edge => {
        const indicator = document.createElement('div');
        indicator.className = `edge-indicator ${edge}`;
        const enabled = edgeSwitching[`${edge}_enabled`];
        if (!enabled) {
            indicator.classList.add('disabled');
        }
        deviceEl.appendChild(indicator);
    });
}

// Handle drag start from palette
function handlePaletteDragStart(e) {
    const deviceId = e.target.dataset.deviceId;
    draggedDevice = { id: deviceId, fromPalette: true };
    e.dataTransfer.effectAllowed = 'copy';
    e.target.style.opacity = '0.5';
    
    // Set up drop zone
    canvas.classList.add('drag-over');
    canvas.addEventListener('dragover', handleCanvasDragOver);
    canvas.addEventListener('drop', handleCanvasDrop);
}

// Handle drag end from palette
function handlePaletteDragEnd(e) {
    e.target.style.opacity = '1';
    canvas.classList.remove('drag-over');
    canvas.removeEventListener('dragover', handleCanvasDragOver);
    canvas.removeEventListener('drop', handleCanvasDrop);
    draggedDevice = null;
}

// Handle drag start from canvas
function handleCanvasDragStart(e) {
    const deviceId = e.target.dataset.deviceId;
    draggedDevice = { id: deviceId, fromPalette: false, element: e.target };
    e.dataTransfer.effectAllowed = 'move';
    e.target.classList.add('dragging');
    
    canvas.addEventListener('dragover', handleCanvasDragOver);
    canvas.addEventListener('drop', handleCanvasDrop);
}

// Handle drag end from canvas
function handleCanvasDragEnd(e) {
    e.target.classList.remove('dragging');
    canvas.removeEventListener('dragover', handleCanvasDragOver);
    canvas.removeEventListener('drop', handleCanvasDrop);
    draggedDevice = null;
}

// Handle drag over canvas
function handleCanvasDragOver(e) {
    e.preventDefault();
    e.dataTransfer.dropEffect = draggedDevice.fromPalette ? 'copy' : 'move';
}

// Handle drop on canvas
function handleCanvasDrop(e) {
    e.preventDefault();
    
    if (!draggedDevice) return;
    
    // Get drop position relative to canvas
    const rect = devicesContainer.getBoundingClientRect();
    const x = Math.max(0, e.clientX - rect.left);
    const y = Math.max(0, e.clientY - rect.top);
    
    // Snap to grid
    const snappedX = Math.round(x / GRID_SIZE) * GRID_SIZE;
    const snappedY = Math.round(y / GRID_SIZE) * GRID_SIZE;
    
    // Convert back to actual coordinates
    const actualX = Math.round(snappedX / DEVICE_SCALE);
    const actualY = Math.round(snappedY / DEVICE_SCALE);
    
    if (draggedDevice.fromPalette) {
        // Adding new device to layout
        addDeviceToLayout(draggedDevice.id, actualX, actualY);
    } else {
        // Moving existing device
        updateDevicePosition(draggedDevice.id, actualX, actualY);
    }
}

// Add device to layout
function addDeviceToLayout(deviceId, x, y) {
    const device = devices.find(d => d.id === deviceId);
    if (!device) return;
    
    // Default resolution if not available
    const width = 1920;
    const height = 1080;
    
    const position = {
        x,
        y,
        width,
        height,
        edge_switching: getDefaultEdgeSwitching()
    };
    
    layoutConfig.devices[deviceId] = position;
    
    // Re-render
    renderDevicePalette();
    renderLayout();
    
    // Select the newly added device
    selectDevice(deviceId);
}

// Update device position
function updateDevicePosition(deviceId, x, y) {
    if (!layoutConfig.devices[deviceId]) return;
    
    layoutConfig.devices[deviceId].x = x;
    layoutConfig.devices[deviceId].y = y;
    
    // Re-render
    renderLayout();
    
    // Update properties panel if this device is selected
    if (selectedDeviceId === deviceId) {
        updatePropertiesPanel();
    }
}

// Select a device
function selectDevice(deviceId) {
    selectedDeviceId = deviceId;
    
    // Update visual selection
    document.querySelectorAll('.canvas-device').forEach(el => {
        el.classList.remove('selected');
        if (el.dataset.deviceId === deviceId) {
            el.classList.add('selected');
        }
    });
    
    // Show and update properties panel
    propertiesPanel.style.display = 'block';
    updatePropertiesPanel();
}

// Deselect device
function deselectDevice() {
    selectedDeviceId = null;
    document.querySelectorAll('.canvas-device').forEach(el => {
        el.classList.remove('selected');
    });
    propertiesPanel.style.display = 'none';
}

// Update properties panel
function updatePropertiesPanel() {
    if (!selectedDeviceId) return;
    
    const device = devices.find(d => d.id === selectedDeviceId);
    const position = layoutConfig.devices[selectedDeviceId];
    
    if (!device || !position) return;
    
    selectedDeviceName.textContent = device.name;
    
    // Update edge switching checkboxes
    const edgeSwitching = position.edge_switching || getDefaultEdgeSwitching();
    edgeLeft.checked = edgeSwitching.left_enabled;
    edgeRight.checked = edgeSwitching.right_enabled;
    edgeTop.checked = edgeSwitching.top_enabled;
    edgeBottom.checked = edgeSwitching.bottom_enabled;
    
    // Update position info
    posX.textContent = position.x;
    posY.textContent = position.y;
    posWidth.textContent = position.width;
    posHeight.textContent = position.height;
}

// Update edge switching configuration
function updateEdgeSwitching() {
    if (!selectedDeviceId) return;
    
    const position = layoutConfig.devices[selectedDeviceId];
    if (!position) return;
    
    position.edge_switching = {
        left_enabled: edgeLeft.checked,
        right_enabled: edgeRight.checked,
        top_enabled: edgeTop.checked,
        bottom_enabled: edgeBottom.checked
    };
    
    // Re-render to update edge indicators
    renderLayout();
    selectDevice(selectedDeviceId); // Maintain selection
}

// Remove selected device from layout
function removeSelectedDevice() {
    if (!selectedDeviceId) return;
    
    delete layoutConfig.devices[selectedDeviceId];
    
    deselectDevice();
    renderDevicePalette();
    renderLayout();
}

// Save layout
async function saveLayout() {
    try {
        await invoke('save_layout_config', { layout: layoutConfig });
        console.log('Layout saved successfully');
        
        // Show success notification
        notificationManager.success(
            'Layout Saved',
            'Device layout configuration has been saved successfully',
            3000
        );
        
        // Show success feedback on button
        saveBtn.textContent = '✓ Saved';
        saveBtn.classList.add('btn-success');
        saveBtn.classList.remove('btn-primary');
        
        setTimeout(() => {
            saveBtn.innerHTML = '<span class="icon">💾</span> Save Layout';
            saveBtn.classList.remove('btn-success');
            saveBtn.classList.add('btn-primary');
        }, 2000);
    } catch (error) {
        console.error('Failed to save layout:', error);
        notificationManager.error(
            'Save Failed',
            'Failed to save layout: ' + error
        );
    }
}

// Reset layout
async function resetLayout() {
    if (!confirm('Are you sure you want to reset the layout? This will remove all devices from the canvas.')) {
        return;
    }
    
    layoutConfig = { devices: {} };
    deselectDevice();
    renderDevicePalette();
    renderLayout();
}

// Go back to main page
function goBack() {
    window.location.href = 'index.html';
}

// Get default edge switching configuration
function getDefaultEdgeSwitching() {
    return {
        left_enabled: true,
        right_enabled: true,
        top_enabled: true,
        bottom_enabled: true
    };
}

// Show error
function showError(message) {
    devicePalette.innerHTML = `
        <div style="color: #ef4444; font-size: 13px;">
            ⚠️ ${escapeHtml(message)}
        </div>
    `;
}

// Escape HTML to prevent XSS
function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}

// Start the app
init();
