# Anni Connect

In Anni choose **Settings → Connect in browser**. The connection page shows the extension folder. In Chrome (`chrome://extensions`) or Edge (`edge://extensions`), enable Developer mode, choose **Load unpacked**, and select that folder. Pin Anni Connect, then refresh Anni's browser connection page. This is a one-time setup for that browser profile; keep the folder in place.

Open INNA in your normal browser profile, use your existing Google login, then click **Anni Connect → Connect this INNA account**. The app validates the session with INNA and resumes your planner. No Google password is requested by Anni. Chrome/Edge may still ask you to authenticate when their own session has expired.

The extension requests cookies access only for `https://nam.inna.is/` and loopback access for `http://127.0.0.1/`. It reads INNA cookies only after you click Connect in its popup. It never reads Google cookies, password stores, browsing history, or other profiles. A 10-minute random capability pairs the extension with a temporary receiver bound exclusively to this computer's loopback interface. Session data goes only to that receiver, never to the diagnostic server. Pairing is held in browser session storage and cleared on success or browser restart; expired requests cannot be used. The extension does not save the transferred cookies.

Uninstall it through your browser's extension manager. To disconnect the app, use Anni's INNA connection settings; signing out in the browser alone does not necessarily revoke an INNA session already copied to the app. This extension is bundled locally and has not been published to an extension store.
