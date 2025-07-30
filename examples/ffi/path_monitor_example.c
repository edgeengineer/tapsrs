/**
 * Example C program demonstrating the Path Monitor FFI
 * 
 * This example shows how to:
 * 1. Create a network path monitor
 * 2. List current network interfaces
 * 3. Watch for network changes
 * 4. Clean up resources
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

// Include the Transport Services header
// In a real application, this would be: #include <transport_services.h>
// For this example, we'll define the necessary structures and functions

typedef void* TransportServicesHandle;

// Interface status enum
typedef enum {
    TRANSPORT_SERVICES_INTERFACE_STATUS_UP = 0,
    TRANSPORT_SERVICES_INTERFACE_STATUS_DOWN = 1,
    TRANSPORT_SERVICES_INTERFACE_STATUS_UNKNOWN = 2
} TransportServicesInterfaceStatus;

// Interface structure
typedef struct {
    char* name;
    uint32_t index;
    char** ips;
    size_t ip_count;
    TransportServicesInterfaceStatus status;
    char* interface_type;
    int is_expensive;
} TransportServicesInterface;

// Change event type enum
typedef enum {
    TRANSPORT_SERVICES_CHANGE_EVENT_ADDED = 0,
    TRANSPORT_SERVICES_CHANGE_EVENT_REMOVED = 1,
    TRANSPORT_SERVICES_CHANGE_EVENT_MODIFIED = 2,
    TRANSPORT_SERVICES_CHANGE_EVENT_PATH_CHANGED = 3
} TransportServicesChangeEventType;

// Change event structure
typedef struct {
    TransportServicesChangeEventType event_type;
    TransportServicesInterface* interface;
    TransportServicesInterface* old_interface;  // For Modified events
    char* description;                          // For PathChanged events
} TransportServicesChangeEvent;

// Function declarations
extern int transport_services_init(void);
extern void transport_services_cleanup(void);
extern const char* transport_services_get_last_error(void);

extern TransportServicesHandle* transport_services_path_monitor_create(void);
extern void transport_services_path_monitor_destroy(TransportServicesHandle* handle);

extern int transport_services_path_monitor_list_interfaces(
    TransportServicesHandle* handle,
    TransportServicesInterface*** interfaces,
    size_t* count
);

extern void transport_services_path_monitor_free_interfaces(
    TransportServicesInterface** interfaces,
    size_t count
);

typedef void (*TransportServicesPathMonitorCallback)(
    const TransportServicesChangeEvent* event,
    void* user_data
);

extern TransportServicesHandle* transport_services_path_monitor_start_watching(
    TransportServicesHandle* handle,
    TransportServicesPathMonitorCallback callback,
    void* user_data
);

extern void transport_services_path_monitor_stop_watching(
    TransportServicesHandle* handle
);

// Helper function to print interface information
void print_interface(const TransportServicesInterface* iface) {
    printf("Interface: %s (index: %u)\n", iface->name, iface->index);
    printf("  Status: %s\n", 
           iface->status == TRANSPORT_SERVICES_INTERFACE_STATUS_UP ? "UP" :
           iface->status == TRANSPORT_SERVICES_INTERFACE_STATUS_DOWN ? "DOWN" : "UNKNOWN");
    printf("  Type: %s\n", iface->interface_type);
    printf("  Expensive: %s\n", iface->is_expensive ? "Yes" : "No");
    
    if (iface->ip_count > 0) {
        printf("  IP Addresses:\n");
        for (size_t i = 0; i < iface->ip_count; i++) {
            printf("    - %s\n", iface->ips[i]);
        }
    }
    printf("\n");
}

// Callback function for network changes
void network_change_callback(const TransportServicesChangeEvent* event, void* user_data) {
    const char* event_name = "";
    
    switch (event->event_type) {
        case TRANSPORT_SERVICES_CHANGE_EVENT_ADDED:
            event_name = "ADDED";
            break;
        case TRANSPORT_SERVICES_CHANGE_EVENT_REMOVED:
            event_name = "REMOVED";
            break;
        case TRANSPORT_SERVICES_CHANGE_EVENT_MODIFIED:
            event_name = "MODIFIED";
            break;
        case TRANSPORT_SERVICES_CHANGE_EVENT_PATH_CHANGED:
            event_name = "PATH_CHANGED";
            break;
    }
    
    printf("=== Network Change Event: %s ===\n", event_name);
    
    switch (event->event_type) {
        case TRANSPORT_SERVICES_CHANGE_EVENT_ADDED:
        case TRANSPORT_SERVICES_CHANGE_EVENT_REMOVED:
            if (event->interface) {
                print_interface(event->interface);
            }
            break;
            
        case TRANSPORT_SERVICES_CHANGE_EVENT_MODIFIED:
            if (event->old_interface) {
                printf("Old interface state:\n");
                print_interface(event->old_interface);
            }
            if (event->interface) {
                printf("New interface state:\n");
                print_interface(event->interface);
            }
            break;
            
        case TRANSPORT_SERVICES_CHANGE_EVENT_PATH_CHANGED:
            if (event->description) {
                printf("Path change: %s\n", event->description);
            }
            break;
    }
    
    printf("================================\n\n");
}

int main(int argc, char* argv[]) {
    // Initialize Transport Services
    if (transport_services_init() != 0) {
        fprintf(stderr, "Failed to initialize Transport Services\n");
        return 1;
    }
    
    // Create a path monitor
    TransportServicesHandle* monitor = transport_services_path_monitor_create();
    if (!monitor) {
        fprintf(stderr, "Failed to create path monitor: %s\n", 
                transport_services_get_last_error());
        transport_services_cleanup();
        return 1;
    }
    
    printf("Network Path Monitor Example\n");
    printf("============================\n\n");
    
    // List current interfaces
    TransportServicesInterface** interfaces = NULL;
    size_t interface_count = 0;
    
    if (transport_services_path_monitor_list_interfaces(monitor, &interfaces, &interface_count) == 0) {
        printf("Current network interfaces (%zu found):\n\n", interface_count);
        
        for (size_t i = 0; i < interface_count; i++) {
            print_interface(interfaces[i]);
        }
        
        // Free the interfaces
        transport_services_path_monitor_free_interfaces(interfaces, interface_count);
    } else {
        fprintf(stderr, "Failed to list interfaces: %s\n", 
                transport_services_get_last_error());
    }
    
    // Start watching for changes
    printf("Starting network change monitoring...\n");
    printf("Try connecting/disconnecting WiFi or changing networks\n");
    printf("Press Ctrl+C to stop\n\n");
    
    TransportServicesHandle* watcher = transport_services_path_monitor_start_watching(
        monitor, 
        network_change_callback, 
        NULL
    );
    
    if (!watcher) {
        fprintf(stderr, "Failed to start watching: %s\n", 
                transport_services_get_last_error());
        transport_services_path_monitor_destroy(monitor);
        transport_services_cleanup();
        return 1;
    }
    
    // Run for 30 seconds
    sleep(30);
    
    // Stop watching
    transport_services_path_monitor_stop_watching(watcher);
    
    // Cleanup
    transport_services_path_monitor_destroy(monitor);
    transport_services_cleanup();
    
    printf("\nMonitoring complete.\n");
    
    return 0;
}