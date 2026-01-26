/**
 * Version Conflict Detection and Recovery
 * 
 * Tracks message versions from the server and detects gaps (missed messages).
 * When a gap is detected, automatically requests a full sync to recover.
 */

import * as state from '../state.js';
import { requestSync } from './live-connection.js';

/**
 * Check for version conflicts and handle recovery
 * 
 * This should be called whenever a message with a version number is received.
 * It compares the received version with the last known version to detect gaps.
 * 
 * @param {number} receivedVersion - The version number from the incoming message
 * @returns {boolean} True if no conflict (version OK), false if gap detected and sync requested
 */
export function checkVersionConflict(receivedVersion) {
    if (typeof receivedVersion !== 'number') {
        console.warn('checkVersionConflict: receivedVersion is not a number:', receivedVersion);
        return true; // No version info, can't detect conflict
    }
    
    const lastVersion = state.get('sync.version') || 0;
    
    // First message or version matches expected (lastVersion + 1 or same)
    if (lastVersion === 0 || receivedVersion === lastVersion + 1 || receivedVersion === lastVersion) {
        return true; // No conflict
    }
    
    // Gap detected: receivedVersion > lastVersion + 1
    if (receivedVersion > lastVersion + 1) {
        const missedCount = receivedVersion - lastVersion - 1;
        console.warn(`Version gap detected: last=${lastVersion}, received=${receivedVersion}, missed=${missedCount} messages`);
        
        // Request full sync to recover
        console.log('Requesting full sync to recover from missed messages...');
        requestSync()
            .then(() => {
                console.log('Full sync requested successfully');
            })
            .catch(error => {
                console.error('Failed to request sync:', error);
            });
        
        return false; // Conflict detected
    }
    
    // receivedVersion < lastVersion (out of order or duplicate)
    // This could happen if messages arrive out of order, but shouldn't be common
    console.warn(`Out-of-order version: last=${lastVersion}, received=${receivedVersion}`);
    return true; // Don't sync for old/duplicate messages
}

/**
 * Reset version tracking
 * Useful when switching connections or after a manual full sync.
 */
export function resetVersionTracking() {
    state.set('sync.version', 0);
    console.log('Version tracking reset');
}
