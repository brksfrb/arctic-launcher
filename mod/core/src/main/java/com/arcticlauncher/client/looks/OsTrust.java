package com.arcticlauncher.client.looks;

import java.io.File;
import java.io.FileInputStream;
import java.io.InputStream;
import java.security.KeyStore;
import java.security.cert.Certificate;
import java.security.cert.CertificateException;
import java.security.cert.CertificateFactory;
import java.security.cert.X509Certificate;
import java.util.ArrayList;
import java.util.List;

import javax.net.ssl.SSLContext;
import javax.net.ssl.SSLSocketFactory;
import javax.net.ssl.TrustManager;
import javax.net.ssl.TrustManagerFactory;
import javax.net.ssl.X509TrustManager;

/**
 * HTTPS that trusts what the operating system trusts, as well as Java's own
 * list. Old Minecraft runs on a 2015 Java that doesn't know today's
 * certificate authorities, and antivirus HTTPS scanning adds its own root to
 * Windows only; either way Arctic's requests would fail without this.
 */
public final class OsTrust {
	/** Where Linux and macOS keep the system's certificates (the first one found). */
	private static final String[] BUNDLES = {
			"/etc/ssl/certs/ca-certificates.crt", "/etc/pki/tls/certs/ca-bundle.crt", "/etc/ssl/ca-bundle.pem",
			"/etc/ssl/cert.pem", "/usr/local/etc/openssl/cert.pem",
	};

	private static SSLSocketFactory factory;
	private static boolean tried;

	private OsTrust() {}

	/**
	 * Use it for every HTTPS request in the game, Minecraft's own too (skins,
	 * joining servers). It trusts everything Java's default does, and more.
	 */
	public static void installDefault() {
		SSLSocketFactory f = factory();
		if (f != null) {
			javax.net.ssl.HttpsURLConnection.setDefaultSSLSocketFactory(f);
		}
	}

	/** The socket factory, or null to use Java's default (if setting up failed). */
	static synchronized SSLSocketFactory factory() {
		if (tried) {
			return factory;
		}
		tried = true;
		try {
			List<X509TrustManager> managers = new ArrayList<X509TrustManager>();
			addManager(managers, null);
			KeyStore os = osStore();
			if (os != null) {
				addManager(managers, os);
			}
			SSLContext context = SSLContext.getInstance("TLS");
			context.init(null, new TrustManager[] {new Either(managers)}, null);
			factory = context.getSocketFactory();
		} catch (Exception e) {
			factory = null;
		}
		return factory;
	}

	private static void addManager(List<X509TrustManager> out, KeyStore store) throws Exception {
		TrustManagerFactory tmf = TrustManagerFactory.getInstance(TrustManagerFactory.getDefaultAlgorithm());
		tmf.init(store);
		for (TrustManager tm : tmf.getTrustManagers()) {
			if (tm instanceof X509TrustManager) {
				out.add((X509TrustManager) tm);
			}
		}
	}

	/** The system's trust store: Windows' own, or the PEM bundle elsewhere. */
	private static KeyStore osStore() {
		String os = System.getProperty("os.name", "").toLowerCase(java.util.Locale.ROOT);
		try {
			if (os.contains("win")) {
				KeyStore store = KeyStore.getInstance("Windows-ROOT");
				store.load(null, null);
				return store;
			}
			for (String path : BUNDLES) {
				File file = new File(path);
				if (file.isFile()) {
					return fromPem(file);
				}
			}
		} catch (Exception e) {
			return null;
		}
		return null;
	}

	private static KeyStore fromPem(File file) throws Exception {
		KeyStore store = KeyStore.getInstance(KeyStore.getDefaultType());
		store.load(null, null);
		CertificateFactory cf = CertificateFactory.getInstance("X.509");
		try (InputStream in = new FileInputStream(file)) {
			int i = 0;
			for (Certificate cert : cf.generateCertificates(in)) {
				store.setCertificateEntry("os-" + i++, cert);
			}
		}
		return store;
	}

	/** Trusted if any of the lists trusts it. */
	private static final class Either implements X509TrustManager {
		private final List<X509TrustManager> managers;

		Either(List<X509TrustManager> managers) {
			this.managers = managers;
		}

		@Override
		public void checkClientTrusted(X509Certificate[] chain, String authType) throws CertificateException {
			CertificateException last = null;
			for (X509TrustManager tm : managers) {
				try {
					tm.checkClientTrusted(chain, authType);
					return;
				} catch (CertificateException e) {
					last = e;
				}
			}
			throw last != null ? last : new CertificateException("no trust manager");
		}

		@Override
		public void checkServerTrusted(X509Certificate[] chain, String authType) throws CertificateException {
			CertificateException last = null;
			for (X509TrustManager tm : managers) {
				try {
					tm.checkServerTrusted(chain, authType);
					return;
				} catch (CertificateException e) {
					last = e;
				}
			}
			throw last != null ? last : new CertificateException("no trust manager");
		}

		@Override
		public X509Certificate[] getAcceptedIssuers() {
			List<X509Certificate> all = new ArrayList<X509Certificate>();
			for (X509TrustManager tm : managers) {
				for (X509Certificate c : tm.getAcceptedIssuers()) {
					all.add(c);
				}
			}
			return all.toArray(new X509Certificate[0]);
		}
	}
}
