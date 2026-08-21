import type { Metadata } from "next";
import { Geist, Geist_Mono } from "next/font/google";
import "./globals.css";

const geistSans = Geist({
  variable: "--font-geist-sans",
  subsets: ["latin"],
});

const geistMono = Geist_Mono({
  variable: "--font-geist-mono",
  subsets: ["latin"],
});

export const metadata: Metadata = {
  metadataBase: new URL("https://quotabar.fyi"),
  title: "QuotaBar — Make your Codex week last",
  description:
    "A private macOS menu-bar guardrail that helps you spread Codex usage across the week.",
  icons: {
    icon: "/icon.png",
    shortcut: "/icon.png",
  },
  openGraph: {
    title: "QuotaBar — Use Codex steadily. Not all at once.",
    description: "A private macOS menu-bar guardrail that helps you spread Codex usage across the week.",
    url: "https://quotabar.fyi",
    siteName: "QuotaBar",
    images: [{ url: "/og.png", width: 1200, height: 630, alt: "QuotaBar for macOS" }],
    type: "website",
  },
  twitter: {
    card: "summary_large_image",
    title: "QuotaBar — Use Codex steadily. Not all at once.",
    description: "A private macOS menu-bar guardrail that helps you spread Codex usage across the week.",
    images: ["/og.png"],
  },
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body
        className={`${geistSans.variable} ${geistMono.variable} antialiased`}
      >
        {children}
      </body>
    </html>
  );
}
