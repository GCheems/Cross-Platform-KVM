const { invoke } = window.__TAURI__.tauri;

// State management
let updateInfo = null;

// DOM elements
const checkingState = document.getElementById('checking-state');
const updateAvailableState = document.getElementById('update-available-state');
const noUpdateState = document.getElementById('no-update-state');
const installingState = document.getElementById('installing-state');
const errorState = document.getElementById('error-state');

// Buttons
const installBtn = document.getElementById('install-btn');
const laterBtn = document.getElementById('later-btn');
const closeBtn = document.getElementById('close-btn');
const retryBtn = document.getElementById('retry-btn');
const cancelBtn = document.getElementById('cancel-btn');

// Initialize
document.addEventListener('DOMContentLoaded', () => {
    checkForUpdates();
    setupEventListeners();
});

function setupEventListeners() {
    installBtn?.addEventListener('click', installUpdate);
    laterBtn?.addEventListener('click', closeWindow);
    closeBtn?.addEventListener('click', closeWindow);
    retryBtn?.addEventListener('click', checkForUpdates);
    cancelBtn?.addEventListener('click', closeWindow);
}

async function checkForUpdates() {
    showState('checking');
    
    try {
        updateInfo = await invoke('check_for_updates');
        
        if (updateInfo.available) {
            displayUpdateAvailable(updateInfo);
        } else {
            displayNoUpdate(updateInfo);
        }
    } catch (error) {
        displayError(error);
    }
}

async function installUpdate() {
    showState('installing');
    
    try {
        await invoke('install_update');
        // The app will restart automatically after successful installation
    } catch (error) {
        displayError(`Failed to install update: ${error}`);
    }
}

function displayUpdateAvailable(info) {
    document.getElementById('current-version').textContent = info.current_version;
    document.getElementById('new-version').textContent = info.version;
    
    if (info.release_date) {
        document.getElementById('release-date').textContent = formatDate(info.release_date);
    } else {
        document.getElementById('release-date').textContent = 'N/A';
    }
    
    if (info.release_notes) {
        const notesContainer = document.getElementById('release-notes-container');
        const notesContent = document.getElementById('release-notes');
        notesContent.textContent = info.release_notes;
        notesContainer.style.display = 'block';
    }
    
    showState('update-available');
}

function displayNoUpdate(info) {
    document.getElementById('current-version-no-update').textContent = info.current_version;
    showState('no-update');
}

function displayError(error) {
    const errorMessage = document.getElementById('error-message');
    errorMessage.textContent = typeof error === 'string' ? error : error.toString();
    showState('error');
}

function showState(state) {
    // Hide all states
    checkingState.style.display = 'none';
    updateAvailableState.style.display = 'none';
    noUpdateState.style.display = 'none';
    installingState.style.display = 'none';
    errorState.style.display = 'none';
    
    // Show requested state
    switch (state) {
        case 'checking':
            checkingState.style.display = 'block';
            break;
        case 'update-available':
            updateAvailableState.style.display = 'block';
            break;
        case 'no-update':
            noUpdateState.style.display = 'block';
            break;
        case 'installing':
            installingState.style.display = 'block';
            break;
        case 'error':
            errorState.style.display = 'block';
            break;
    }
}

function formatDate(dateString) {
    try {
        const date = new Date(dateString);
        return date.toLocaleDateString(undefined, {
            year: 'numeric',
            month: 'long',
            day: 'numeric'
        });
    } catch {
        return dateString;
    }
}

function closeWindow() {
    window.close();
}
