"""Read the desktop's static GNOME proxy when the build has no proxy environment."""
import json

settings = {}
try:
    from gi.repository import Gio
except ImportError:
    Gio = None
if Gio:
    source = Gio.SettingsSchemaSource.get_default()
    schema = source.lookup('org.gnome.system.proxy', True) if source else None
    if schema:
        proxy = Gio.Settings.new_full(schema, None, None)
        if proxy.get_string('mode') == 'manual':
            for proto, key in [('http', 'http_proxy'), ('https', 'https_proxy'), ('socks', 'all_proxy')]:
                config = proxy.get_child(proto)
                host = config.get_string('host')
                if host:
                    if ':' in host and not host.startswith('['):
                        host = f'[{host}]'
                    scheme = 'socks5h' if proto == 'socks' else 'http'
                    settings[key] = f'{scheme}://{host}:{config.get_int("port")}'
            if proxy.get_boolean('use-same-proxy') and 'http_proxy' in settings:
                settings['https_proxy'] = settings['http_proxy']
            settings['no_proxy'] = ','.join(host[1:] if host.startswith('*.') else host
                                            for host in proxy.get_strv('ignore-hosts'))
print(json.dumps(settings))
