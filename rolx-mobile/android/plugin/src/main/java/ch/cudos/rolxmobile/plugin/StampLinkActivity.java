package ch.cudos.rolxmobile.plugin;

import android.app.Activity;
import android.content.Intent;
import android.net.Uri;
import android.os.Bundle;

import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;

/**
 * Receives the stamp links ({@code rolx://stamp?...}) from the app shortcuts, a scanned QR code, Tasker or any other
 * app, queues them for the Rust app and brings the app to the front. The app's own activity can't take them: dx
 * generates it, and it never looks at intents that arrive while it runs ({@code singleTask}).
 *
 * <p>The queue is a file in the files dir, one line per link ({@code <epoch millis> <link>}), so a link that starts
 * the app is not lost. The app reads it at start and whenever {@link #linkQueued} wakes it up; see
 * {@code src/links.rs}.
 */
public class StampLinkActivity extends Activity {
    /** Read and emptied by the app, see {@code QUEUE_FILE} in {@code src/links.rs}. */
    static final String QUEUE_FILE = "stamp-links.txt";

    /** Registered by the running app, see {@code android::listen_for_stamp_links}. */
    private static native void linkQueued();

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        Uri link = getIntent().getData();
        if (link != null && queue(link.toString())) {
            try {
                linkQueued();
            } catch (UnsatisfiedLinkError e) {
                // The app is not running yet; it reads the queue when it starts.
            }
        }
        Intent open = getPackageManager().getLaunchIntentForPackage(getPackageName());
        if (open != null) {
            open.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
            startActivity(open);
        }
        finish();
    }

    /** Appends the link to the queue; returns whether it was written. */
    private boolean queue(String link) {
        if (link.indexOf('\n') >= 0 || link.indexOf('\r') >= 0) {
            return false;
        }
        String line = System.currentTimeMillis() + " " + link + "\n";
        try (FileOutputStream out = new FileOutputStream(new File(getFilesDir(), QUEUE_FILE), true)) {
            out.write(line.getBytes(StandardCharsets.UTF_8));
            return true;
        } catch (IOException e) {
            return false;
        }
    }
}
