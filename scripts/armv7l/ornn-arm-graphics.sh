#!/bin/sh
set -eu
# The generic HAL requests eight-bit dumb buffers. Virtio requires the GBM
# backend, whose packaged installation script selects it on the HAL partition.
rpm -U --replacepkgs /etc/ornn-arm-tbm.rpm
# The software EGL driver lacks the multisample config used by Impeller.
db=/opt/dbspace/.pkgmgr_parser.db
if [ -x /usr/apps/org.tizen.homescreen/bin/runner ] && [ -f "$db" ]; then
    # TCG can take longer than AMD's launch deadline to draw the first frame.
    # Use the platform app's lifecycle metadata only for the stock Home and TaskBar apps.
    sqlite3 -cmd '.timeout 5000' "$db" "BEGIN IMMEDIATE;
        UPDATE package_app_app_metadata SET md_value='false'
        WHERE app_id='org.tizen.homescreen' AND md_value='true'
        AND md_key='http://tizen.org/metadata/flutter_tizen/enable_impeller';
        INSERT OR IGNORE INTO package_app_app_metadata(app_id,md_key,md_value)
            SELECT app_id,'http://tizen.org/metadata/appmanager/anr_check_bypass','yes'
            FROM package_app_info WHERE app_id IN ('org.tizen.homescreen','org.tizen.taskbar');
        COMMIT;"
fi
rm -f /etc/ornn-arm-tbm.rpm
