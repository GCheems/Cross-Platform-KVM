// Tauri API
const { invoke } = window.__TAURI__.tauri;

// State
let hotkeys = {};
let devices = [];
let currentEditingId = null;
let recordedKeyCode = null;
let recordedModifiers = { shift: false, ctrl: false, alt: false, meta: false };
let isRecording = false;

// DOM Elements
const hotkeyList = document.getElementById('hotkey-list');
const emptyState = document.getElementById('empty-state');
const backBtn = document.getElementById('back-btn');
const addHotkeyBtn = document.getElementById('add-hotkey-btn');
const addFirstHotkeyBtn = document.getElementById('add-first-hotkey-btn');
const statusText = document.getElementById('status-text');

// Modal elements
const hotkeyModal = document.getElementById('hotkey-modal');
const modalTitle = document.getElementById('modal-title');
const closeModalBtn = document.getElementById('close-modal-btn');
const deviceSelect = document.getElementById('device-select');
const hotkeyRecorder = document.getElementById('hotkey-recorder');
const recorderDisplay = document.getElementById('recorder-display');
const clearHotkeyBtn = document.getElementById('clear-hotkey-btn');
const enabledCheckbox = document.getElementById('enabled-checkbox');
const conflictWarning = document.getElementById('conflict-warning');
const conflictMessage = document.getElementById('conflict-message');
const cancelBtn = document.getElementById('cancel-btn');
const saveHotkeyBtn = document.getElementById('save-hotkey-btn');

// Key code to name mapping
const KEY_NAMES = {
    8: 'Backspace', 9: 'Tab', 13: 'Enter', 16: 'Shift', 17: 'Ctrl', 18: 'Alt',
    20: 'CapsLock', 27: 'Escape', 32: 'Space', 33: 'PageUp', 34: 'PageDown',
    35: 'End', 36: 'Home', 37: 'Left', 38: 'Up', 39: 'Right', 40: 'Down',
    45: 'Insert', 46: 'Delete', 91: 'Meta', 93: 'Meta',
    112: 'F1', 113: 'F2', 114: 'F3', 115: 'F4', 116: 'F5', 117: 'F6',
    118: 'F7', 119: 'F8', 120: 'F9', 121: 'F10', 122: 'F11', 123: 'F12',
    186: ';', 187: '=', 188: ',', 189: '-', 190: '.', 191: '/', 192: '`',
    219: '[', 220: '\\', 221: ']', 222: "'"
};

// Initialize
async function init() {
    console.log('Initializing Hotkey Configuration UI...');
    
    // Set up event listeners
    backBtn.addEventListener('click', () => window.location.href = 'index.html');
    addHotkeyBtn.addEventListener('click', () => openModal());
    addFirstHotkeyBtn.addEventListener('click', () => openModal());
    closeModalBtn.addEventListener('click', closeModal);
    cancelBtn.addEventListener('click', closeModal);
    saveHotkeyBtn.addEventListener('click', saveHotkey);
    clearHotkeyBtn.addEventListener('click', clearRecording);
    
    // Set up recorder
    recorderDisplay.addEventListener('click', startRecording);
    recorderDisplay.addEventListener('keydown', handleKeyDown);
    recorderDisplay.addEventListener('keyup', handleKeyUp);
    recorderDisplay.setAttribute('tabindex', '0');
    
    // Device select change
    deviceSelect.addEventListener('change', validateForm);
    
    // Load data
    await loadDevices();
    await loadHotkeys();
}

// Load devices
async function loadDevices() {
    try {
        devices = await invoke('get_devices');
        console.log('Loaded devices:', devices);
        populateDeviceSelect();
    } catch (error) {
        console.error('Failed to load devices:', error);
        showError('Failed to load devices: ' + error);
    }
}

// Populate device select dropdown
function populateDeviceSelect() {
    deviceSelect.innerHTML = '<option value="">Select a device...</option>';
    devices.forEach(device => {
        const option = document.createElement('option');
        option.value = device.id;
        option.textContent = `${device.name} (${device.os_type})`;
        deviceSelect.appendChild(option);
    });
}

// Load hotkeys
async function loadHotkeys() {
    try {
        hotkeys = await invoke('get_hotkeys');
        console.log('Loaded hotkeys:', hotkeys);
        renderHotkeys();
        updateStatus(`${Object.keys(hotkeys).length} hotkey(s) configured`);
    } catch (error) {
        console.error('Failed to load hotkeys:', error);
        showError('Failed to load hotkeys: ' + error);
    }
}

// Render hotkeys
function renderHotkeys() {
    const hotkeyEntries = Object.entries(hotkeys);
    
    if (hotkeyEntries.length === 0) {
        hotkeyList.style.display = 'none';
        emptyState.style.display = 'block';
        return;
    }
    
    hotkeyList.style.display = 'flex';
    emptyState.style.display = 'none';
    
    hotkeyList.innerHTML = hotkeyEntries.map(([id, hotkey]) => 
        createHotkeyCard(id, hotkey)
    ).join('');
    
    // Add event listeners
    hotkeyEntries.forEach(([id, hotkey]) => {
        const editBtn = document.getElementById(`edit-${id}`);
        const deleteBtn = document.getElementById(`delete-${id}`);
        const toggleBtn = document.getElementById(`toggle-${id}`);
        
        if (editBtn) editBtn.addEventListener('click', () => editHotkey(id));
        if (deleteBtn) deleteBtn.addEventListener('click', () => deleteHotkey(id));
        if (toggleBtn) toggleBtn.addEventListener('click', () => toggleHotkey(id));
    });
}

// Create hotkey card HTML
function createHotkeyCard(id, hotkey) {
    const device = devices.find(d => d.id === hotkey.device_id);
    const deviceName = device ? device.name : hotkey.device_id;
    const keys = formatHotkeyDisplay(hotkey.key_code, hotkey.modifiers);
    
    return `
        <div class="hotkey-card ${hotkey.enabled ? '' : 'disabled'} fade-in">
            <div class="hotkey-info">
                <div class="hotkey-header">
                    <div class="hotkey-device">${escapeHtml(deviceName)}</div>
                    <span class="hotkey-badge ${hotkey.enabled ? 'enabled' : 'disabled'}">
                        ${hotkey.enabled ? '✓ Enabled' : '✗ Disabled'}
                    </span>
                </div>
                <div class="hotkey-combination">
                    ${keys}
                </div>
            </div>
            <div class="hotkey-actions">
                <button id="toggle-${id}" class="btn btn-secondary btn-sm" title="${hotkey.enabled ? 'Disable' : 'Enable'}">
                    ${hotkey.enabled ? '⏸' : '▶'}
                </button>
                <button id="edit-${id}" class="btn btn-secondary btn-sm">
                    <span class="icon">✏️</span>
                    Edit
                </button>
                <button id="delete-${id}" class="btn btn-danger btn-sm">
                    <span class="icon">🗑️</span>
                    Delete
                </button>
            </div>
        </div>
    `;
}

// Format hotkey for display
function formatHotkeyDisplay(keyCode, modifiers) {
    const keys = [];
    
    if (modifiers.ctrl) keys.push('<span class="key-badge modifier">Ctrl</span>');
    if (modifiers.alt) keys.push('<span class="key-badge modifier">Alt</span>');
    if (modifiers.shift) keys.push('<span class="key-badge modifier">Shift</span>');
    if (modifiers.meta) {
        const metaName = navigator.platform.includes('Mac') ? 'Cmd' : 'Win';
        keys.push(`<span class="key-badge modifier">${metaName}</span>`);
    }
    
    const keyName = getKeyName(keyCode);
    keys.push(`<span class="key-badge">${escapeHtml(keyName)}</span>`);
    
    return keys.join('');
}

// Get key name from key code
function getKeyName(keyCode) {
    if (KEY_NAMES[keyCode]) {
        return KEY_NAMES[keyCode];
    }
    
    // For letter keys (65-90)
    if (keyCode >= 65 && keyCode <= 90) {
        return String.fromCharCode(keyCode);
    }
    
    // For number keys (48-57)
    if (keyCode >= 48 && keyCode <= 57) {
        return String.fromCharCode(keyCode);
    }
    
    return `Key${keyCode}`;
}

// Open modal for adding/editing hotkey
function openModal(hotkeyId = null) {
    currentEditingId = hotkeyId;
    
    if (hotkeyId) {
        // Edit mode
        modalTitle.textContent = 'Edit Hotkey';
        const hotkey = hotkeys[hotkeyId];
        deviceSelect.value = hotkey.device_id;
        recordedKeyCode = hotkey.key_code;
        recordedModifiers = { ...hotkey.modifiers };
        enabledCheckbox.checked = hotkey.enabled;
        updateRecorderDisplay();
    } else {
        // Add mode
        modalTitle.textContent = 'Add Hotkey';
        deviceSelect.value = '';
        clearRecording();
        enabledCheckbox.checked = true;
    }
    
    conflictWarning.style.display = 'none';
    hotkeyModal.style.display = 'flex';
    validateForm();
}

// Close modal
function closeModal() {
    hotkeyModal.style.display = 'none';
    currentEditingId = null;
    clearRecording();
}

// Start recording hotkey
function startRecording() {
    isRecording = true;
    recorderDisplay.classList.add('recording');
    recorderDisplay.focus();
}

// Handle key down during recording
function handleKeyDown(event) {
    if (!isRecording) return;
    
    event.preventDefault();
    event.stopPropagation();
    
    // Don't record modifier-only presses
    if ([16, 17, 18, 91, 93].includes(event.keyCode)) {
        return;
    }
    
    recordedKeyCode = event.keyCode;
    recordedModifiers = {
        shift: event.shiftKey,
        ctrl: event.ctrlKey,
        alt: event.altKey,
        meta: event.metaKey
    };
    
    updateRecorderDisplay();
    checkConflict();
    validateForm();
    
    isRecording = false;
    recorderDisplay.classList.remove('recording');
}

// Handle key up during recording
function handleKeyUp(event) {
    if (isRecording) {
        event.preventDefault();
        event.stopPropagation();
    }
}

// Update recorder display
function updateRecorderDisplay() {
    if (recordedKeyCode === null) {
        recorderDisplay.innerHTML = '<span class="placeholder">Press keys to record...</span>';
    } else {
        recorderDisplay.innerHTML = formatHotkeyDisplay(recordedKeyCode, recordedModifiers);
    }
}

// Clear recording
function clearRecording() {
    recordedKeyCode = null;
    recordedModifiers = { shift: false, ctrl: false, alt: false, meta: false };
    isRecording = false;
    recorderDisplay.classList.remove('recording');
    updateRecorderDisplay();
    conflictWarning.style.display = 'none';
    validateForm();
}

// Check for hotkey conflicts
async function checkConflict() {
    if (recordedKeyCode === null) {
        conflictWarning.style.display = 'none';
        return;
    }
    
    try {
        const conflict = await invoke('check_hotkey_conflict', {
            keyCode: recordedKeyCode,
            modifiers: recordedModifiers
        });
        
        if (conflict) {
            // If we're editing and the conflict is with ourselves, ignore it
            if (currentEditingId && conflict.hotkey_id === currentEditingId) {
                conflictWarning.style.display = 'none';
                return;
            }
            
            const device = devices.find(d => d.id === conflict.device_id);
            const deviceName = device ? device.name : conflict.device_id;
            
            conflictMessage.textContent = `This hotkey is already assigned to "${deviceName}"`;
            conflictWarning.style.display = 'flex';
        } else {
            conflictWarning.style.display = 'none';
        }
    } catch (error) {
        console.error('Failed to check conflict:', error);
    }
}

// Validate form
function validateForm() {
    const deviceSelected = deviceSelect.value !== '';
    const hotkeyRecorded = recordedKeyCode !== null;
    const hasConflict = conflictWarning.style.display !== 'none';
    
    saveHotkeyBtn.disabled = !deviceSelected || !hotkeyRecorded || hasConflict;
}

// Save hotkey
async function saveHotkey() {
    const deviceId = deviceSelect.value;
    const enabled = enabledCheckbox.checked;
    
    if (!deviceId || recordedKeyCode === null) {
        return;
    }
    
    const hotkey = {
        key_code: recordedKeyCode,
        modifiers: recordedModifiers,
        device_id: deviceId,
        enabled: enabled
    };
    
    try {
        const hotkeyId = currentEditingId || `hotkey_${Date.now()}`;
        await invoke('save_hotkey', { hotkeyId, hotkey });
        
        console.log('Hotkey saved:', hotkeyId);
        updateStatus('Hotkey saved successfully');
        
        // Show success notification
        const device = devices.find(d => d.id === deviceId);
        const deviceName = device ? device.name : 'Unknown Device';
        notificationManager.success(
            'Hotkey Saved',
            `Hotkey for ${deviceName} has been saved successfully`,
            3000
        );
        
        closeModal();
        await loadHotkeys();
    } catch (error) {
        console.error('Failed to save hotkey:', error);
        notificationManager.error(
            'Save Failed',
            'Failed to save hotkey: ' + error
        );
    }
}

// Edit hotkey
function editHotkey(hotkeyId) {
    openModal(hotkeyId);
}

// Delete hotkey
async function deleteHotkey(hotkeyId) {
    if (!confirm('Are you sure you want to delete this hotkey?')) {
        return;
    }
    
    try {
        await invoke('remove_hotkey', { hotkeyId });
        console.log('Hotkey deleted:', hotkeyId);
        updateStatus('Hotkey deleted');
        
        // Show success notification
        notificationManager.info(
            'Hotkey Deleted',
            'The hotkey has been removed',
            3000
        );
        
        await loadHotkeys();
    } catch (error) {
        console.error('Failed to delete hotkey:', error);
        notificationManager.error(
            'Delete Failed',
            'Failed to delete hotkey: ' + error
        );
    }
}

// Toggle hotkey enabled/disabled
async function toggleHotkey(hotkeyId) {
    const hotkey = hotkeys[hotkeyId];
    if (!hotkey) return;
    
    hotkey.enabled = !hotkey.enabled;
    
    try {
        await invoke('save_hotkey', { hotkeyId, hotkey });
        console.log('Hotkey toggled:', hotkeyId);
        updateStatus(`Hotkey ${hotkey.enabled ? 'enabled' : 'disabled'}`);
        await loadHotkeys();
    } catch (error) {
        console.error('Failed to toggle hotkey:', error);
        alert('Failed to toggle hotkey: ' + error);
    }
}

// Show error
function showError(message) {
    hotkeyList.innerHTML = `
        <div class="loading">
            <p style="color: #ef4444;">⚠️ ${escapeHtml(message)}</p>
        </div>
    `;
}

// Update status
function updateStatus(text) {
    statusText.textContent = text;
}

// Escape HTML to prevent XSS
function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}

// Start the app
init();
