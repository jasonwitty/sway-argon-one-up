pragma Singleton
import QtQuick
import "Numbers.js" as Numbers

// English-only stand-in for Omarchy's I18n singleton, which loads per-language
// catalogs from an Omarchy install. The strings are the `network.*` and
// `wifiqr.*` entries of Omarchy's default/i18n/en/shell.json (MIT, see
// LICENSE-omarchy), so the panel code can keep calling I18n.t() unchanged.
QtObject {
  id: root

  readonly property bool rightToLeft: false

  function t(key, fallback) {
    var value = root.strings[key]
    if (typeof value === "string" && value.length > 0) return value
    return fallback !== undefined ? fallback : key
  }

  function formatNumber(value, decimals) {
    return Numbers.format(value, decimals, ".", "0")
  }

  readonly property var strings: ({
    "network.automatic": "AUTOMATIC",
    "network.bandAuto": "Auto",
    "network.bandAutoTooltip": "Let Wi-Fi pick the band",
    "network.connect": "Connect",
    "network.connected": "Connected",
    "network.connecting": "Connecting…",
    "network.connectionFailed": "Connection failed",
    "network.copyGateway": "Copy gateway",
    "network.copyIp": "Copy IP",
    "network.copyToClipboard": "Copy to clipboard",
    "network.disconnected": "Disconnected",
    "network.disconnecting": "Disconnecting…",
    "network.dnsCustom": "Custom",
    "network.dnsFromDhcp": "Use DNS from DHCP",
    "network.dnsProvider": "DNS PROVIDER",
    "network.downloaded": "Downloaded",
    "network.ethernet": "Ethernet",
    "network.failed": "Failed",
    "network.failedToConnect": "Failed to connect",
    "network.forgetNetwork": "Forget network",
    "network.forgetting": "Forgetting…",
    "network.gateway": "Gateway",
    "network.hidden": "Hidden",
    "network.identity": "Identity (user@domain)",
    "network.ipAddress": "IP Address",
    "network.knownNetworks": "KNOWN NETWORKS",
    "network.limitedAccess": "Limited internet access",
    "network.limitedAccessHeader": "LIMITED INTERNET ACCESS",
    "network.networkLost": "Network lost",
    "network.noConnection": "No connection",
    "network.notConnected": "NOT CONNECTED",
    "network.openCaptivePortal": "Sign into network",
    "network.otherNetworks": "OTHER NETWORKS",
    "network.packetLoss": "Packet Loss",
    "network.passphrase": "Passphrase",
    "network.passphraseRequired": "Passphrase required",
    "network.phrase1": "Wiring bits",
    "network.phrase2": "Handling packets",
    "network.phrase3": "Sorting frames",
    "network.phrase4": "Hauling bytes",
    "network.phrase5": "Routing crumbs",
    "network.phrase6": "Counting collisions",
    "network.phrase7": "Bending light",
    "network.ping": "Ping",
    "network.receiving": "Receiving",
    "network.scanningWifi": "SCANNING WI-FI…",
    "network.sending": "Sending",
    "network.setCustomDns": "Set custom DNS servers",
    "network.setDnsTo": "Set DNS to %1",
    "network.showQrCode": "Show QR code",
    "network.signInRequired": "Sign-in required",
    "network.signInToNetwork": "Sign in to this network",
    "network.stayOnBand": "Stay on %1",
    "network.timedOutConnecting": "Timed out connecting",
    "network.timedOutDisconnecting": "Timed out disconnecting",
    "network.timedOutForgetting": "Timed out forgetting",
    "network.timeout": "Timeout",
    "network.turnWifiOff": "Turn Wi-Fi off",
    "network.turnWifiOn": "Turn Wi-Fi on",
    "network.uploaded": "Uploaded",
    "network.wifiBand": "WI-FI BAND",
    "network.wifiBandCurrent": "WI-FI BAND: %1",
    "network.wrongPassword": "Wrong password",
    "wifiqr.generateFailed": "Could not generate the Wi-Fi QR code",
    "wifiqr.generating": "Generating QR code…",
    "wifiqr.passwordFailed": "Could not read the Wi-Fi password",
    "wifiqr.scanToJoin": "Scan to join this network",
    "wifiqr.showPassword": "Show password"

  })
}
