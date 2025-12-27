// Notification System

class NotificationManager {
    constructor() {
        this.container = null;
        this.notifications = new Map();
        this.notificationId = 0;
        this.init();
    }

    init() {
        // Create notification container
        this.container = document.createElement('div');
        this.container.className = 'notification-container';
        document.body.appendChild(this.container);
    }

    /**
     * Show a notification
     * @param {string} type - Type of notification: 'success', 'error', 'warning', 'info'
     * @param {string} title - Notification title
     * @param {string} message - Notification message
     * @param {number} duration - Duration in milliseconds (0 for persistent)
     */
    show(type, title, message, duration = 5000) {
        const id = this.notificationId++;
        
        // Create notification element
        const notification = document.createElement('div');
        notification.className = `notification ${type}`;
        notification.dataset.id = id;
        
        // Icon based on type
        const icons = {
            success: '✓',
            error: '✕',
            warning: '⚠',
            info: 'ℹ'
        };
        
        notification.innerHTML = `
            <div class="notification-icon">${icons[type] || icons.info}</div>
            <div class="notification-content">
                <div class="notification-title">${this.escapeHtml(title)}</div>
                <div class="notification-message">${this.escapeHtml(message)}</div>
            </div>
            <button class="notification-close" aria-label="Close">×</button>
        `;
        
        // Add close button handler
        const closeBtn = notification.querySelector('.notification-close');
        closeBtn.addEventListener('click', () => this.hide(id));
        
        // Add to container
        this.container.appendChild(notification);
        this.notifications.set(id, notification);
        
        // Auto-hide after duration
        if (duration > 0) {
            setTimeout(() => this.hide(id), duration);
        }
        
        return id;
    }

    /**
     * Hide a notification
     * @param {number} id - Notification ID
     */
    hide(id) {
        const notification = this.notifications.get(id);
        if (!notification) return;
        
        notification.classList.add('fade-out');
        
        setTimeout(() => {
            if (notification.parentNode) {
                notification.parentNode.removeChild(notification);
            }
            this.notifications.delete(id);
        }, 300);
    }

    /**
     * Show a success notification
     */
    success(title, message, duration) {
        return this.show('success', title, message, duration);
    }

    /**
     * Show an error notification
     */
    error(title, message, duration = 0) {
        return this.show('error', title, message, duration);
    }

    /**
     * Show a warning notification
     */
    warning(title, message, duration) {
        return this.show('warning', title, message, duration);
    }

    /**
     * Show an info notification
     */
    info(title, message, duration) {
        return this.show('info', title, message, duration);
    }

    /**
     * Clear all notifications
     */
    clearAll() {
        this.notifications.forEach((_, id) => this.hide(id));
    }

    escapeHtml(text) {
        const div = document.createElement('div');
        div.textContent = text;
        return div.innerHTML;
    }
}

// Edge Highlight Manager
class EdgeHighlightManager {
    constructor() {
        this.activeHighlights = new Map();
    }

    /**
     * Show edge highlight
     * @param {string} edge - Edge position: 'top', 'bottom', 'left', 'right'
     * @param {number} duration - Duration in milliseconds
     */
    show(edge, duration = 500) {
        // Remove existing highlight for this edge
        this.hide(edge);
        
        // Create highlight element
        const highlight = document.createElement('div');
        highlight.className = `edge-highlight-overlay ${edge}`;
        document.body.appendChild(highlight);
        
        this.activeHighlights.set(edge, highlight);
        
        // Auto-remove after duration
        setTimeout(() => this.hide(edge), duration);
    }

    /**
     * Hide edge highlight
     * @param {string} edge - Edge position
     */
    hide(edge) {
        const highlight = this.activeHighlights.get(edge);
        if (highlight && highlight.parentNode) {
            highlight.parentNode.removeChild(highlight);
            this.activeHighlights.delete(edge);
        }
    }

    /**
     * Clear all highlights
     */
    clearAll() {
        this.activeHighlights.forEach((_, edge) => this.hide(edge));
    }
}

// Active Device Indicator
class ActiveDeviceIndicator {
    constructor() {
        this.indicator = null;
        this.hideTimeout = null;
        this.init();
    }

    init() {
        // Create indicator element
        this.indicator = document.createElement('div');
        this.indicator.className = 'active-device-indicator hidden';
        this.indicator.innerHTML = `
            <span class="indicator-dot"></span>
            <span class="indicator-text">Local Device</span>
        `;
        document.body.appendChild(this.indicator);
    }

    /**
     * Show active device indicator
     * @param {string} deviceName - Name of the active device
     * @param {number} duration - Duration to show (0 for persistent)
     */
    show(deviceName, duration = 3000) {
        // Clear any existing hide timeout
        if (this.hideTimeout) {
            clearTimeout(this.hideTimeout);
            this.hideTimeout = null;
        }
        
        // Update text
        const textElement = this.indicator.querySelector('.indicator-text');
        textElement.textContent = deviceName || 'Local Device';
        
        // Show indicator
        this.indicator.classList.remove('hidden');
        
        // Auto-hide after duration
        if (duration > 0) {
            this.hideTimeout = setTimeout(() => this.hide(), duration);
        }
    }

    /**
     * Hide active device indicator
     */
    hide() {
        this.indicator.classList.add('hidden');
        if (this.hideTimeout) {
            clearTimeout(this.hideTimeout);
            this.hideTimeout = null;
        }
    }

    /**
     * Update device name without showing/hiding
     */
    update(deviceName) {
        const textElement = this.indicator.querySelector('.indicator-text');
        textElement.textContent = deviceName || 'Local Device';
    }
}

// Export instances
const notificationManager = new NotificationManager();
const edgeHighlightManager = new EdgeHighlightManager();
const activeDeviceIndicator = new ActiveDeviceIndicator();

// Export for use in other modules
if (typeof window !== 'undefined') {
    window.notificationManager = notificationManager;
    window.edgeHighlightManager = edgeHighlightManager;
    window.activeDeviceIndicator = activeDeviceIndicator;
}
