#include <stdio.h>
#include <string.h>
#include "target/taps.h"

int main() {
    Preconnection *preconnection = taps_preconnection_create();
    RemoteEndpoint *remote_endpoint = taps_remote_endpoint_create();
    taps_remote_endpoint_with_hostname(remote_endpoint, "example.com");
    taps_remote_endpoint_with_port(remote_endpoint, 443);
    taps_preconnection_add_remote_endpoint(preconnection, remote_endpoint);

    Connection *connection = taps_connection_initiate(preconnection);
    if (!connection) {
        printf("Failed to initiate connection\n");
        return 1;
    }

    const char *request = "GET / HTTP/1.1\r\nHost: example.com\r\nConnection: close\r\n\r\n";
    taps_connection_send(connection, (const uint8_t *)request, strlen(request));

    char buffer[4096];
    ssize_t received;
    while ((received = taps_connection_receive(connection, (uint8_t *)buffer, sizeof(buffer) - 1)) > 0) {
        buffer[received] = '\0';
        printf("%s", buffer);
    }

    taps_connection_close(connection);

    return 0;
} 