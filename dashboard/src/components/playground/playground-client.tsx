"use client";

import { useState } from "react";
import { Send, Play, Loader2, Save, Trash2, Plus } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import useSWR from "swr";
import { swrFetcher, Token } from "@/lib/api";
import { toast } from "sonner";

export function PlaygroundClient() {
    const [method, setMethod] = useState("POST");
    const [url, setUrl] = useState("http://localhost:8443/v1/chat/completions");
    const [headers, setHeaders] = useState("Content-Type: application/json");
    const [body, setBody] = useState(`{
  "model": "gpt-4.1-mini",
  "messages": [
    {"role": "user", "content": "Hello world"}
  ]
}`);
    const [response, setResponse] = useState<string | null>(null);
    const [responseStatus, setResponseStatus] = useState<number | null>(null);
    const [loading, setLoading] = useState(false);
    const [selectedTokenId, setSelectedTokenId] = useState<string>("");

    const { data: tokens } = useSWR<Token[]>("/tokens", swrFetcher);

    // SEC-02/09: Gateway URL for validating playground requests.
    // Only inject token auth for URLs that match the gateway origin.
    const GATEWAY_URL = process.env.NEXT_PUBLIC_GATEWAY_URL || "http://localhost:8443";

    const isGatewayUrl = (targetUrl: string): boolean => {
        try {
            const target = new URL(targetUrl);
            const gateway = new URL(GATEWAY_URL);
            return target.origin === gateway.origin;
        } catch {
            return false;
        }
    };

    const handleSend = async () => {
        if (!selectedTokenId) {
            toast.error("Please select a token first");
            return;
        }

        const sendingToGateway = isGatewayUrl(url);
        if (!sendingToGateway) {
            toast.warning("Request is not targeting your gateway — Authorization header will NOT be sent to prevent token leakage.");
        }

        setLoading(true);
        setResponse(null);
        setResponseStatus(null);

        try {
            // Parse headers
            const headerObj: Record<string, string> = {};
            headers.split("\n").forEach(line => {
                const [key, value] = line.split(":").map(s => s.trim());
                if (key && value) headerObj[key] = value;
            });

            // SEC-09: Only inject auth for gateway URLs to prevent token exfiltration
            if (sendingToGateway) {
                const token = tokens?.find(t => t.id === selectedTokenId);
                if (token) {
                    headerObj["Authorization"] = `Bearer ${token.id}`;
                }
            }

            const res = await fetch(url, {
                method,
                headers: headerObj,
                body: method !== "GET" ? body : undefined,
            });

            setResponseStatus(res.status);

            const contentType = res.headers.get("content-type");
            if (contentType && contentType.includes("application/json")) {
                const data = await res.json();
                setResponse(JSON.stringify(data, null, 2));
            } else {
                const text = await res.text();
                setResponse(text);
            }
        } catch (e: unknown) {
            const err = e as Error;
            toast.error("Request failed: " + err.message);
            setResponse("Error: " + err.message);
        } finally {
            setLoading(false);
        }
    };

    return (
        <div className="grid lg:grid-cols-2 gap-3 h-[calc(100vh-140px)]">
            {/* Request Panel */}
            <Card className="flex flex-col h-full border-border/60 shadow-sm overflow-hidden">
                <CardHeader className="py-3 px-4 border-b bg-muted/20 flex flex-row items-center justify-between">
                    <div className="flex items-center gap-2">
                        <Play className="h-4 w-4 text-primary" />
                        <span className="font-semibold text-sm">Request</span>
                    </div>
                    <div className="w-[200px]">
                        <Select value={selectedTokenId} onValueChange={setSelectedTokenId}>
                            <SelectTrigger className="h-8 text-xs">
                                <SelectValue placeholder="Select Token..." />
                            </SelectTrigger>
                            <SelectContent>
                                {tokens?.map(t => (
                                    <SelectItem key={t.id} value={t.id} className="text-xs">
                                        {t.name}
                                    </SelectItem>
                                ))}
                            </SelectContent>
                        </Select>
                    </div>
                </CardHeader>
                <CardContent className="flex-1 p-0 flex flex-col min-h-0">
                    <div className="p-4 space-y-4 border-b">
                        <div className="flex gap-2">
                            <Select value={method} onValueChange={setMethod}>
                                <SelectTrigger className="w-[100px] font-mono font-semibold">
                                    <SelectValue />
                                </SelectTrigger>
                                <SelectContent>
                                    <SelectItem value="GET">GET</SelectItem>
                                    <SelectItem value="POST">POST</SelectItem>
                                    <SelectItem value="PUT">PUT</SelectItem>
                                    <SelectItem value="DELETE">DELETE</SelectItem>
                                </SelectContent>
                            </Select>
                            <Input
                                value={url}
                                onChange={(e: React.ChangeEvent<HTMLInputElement>) => setUrl(e.target.value)}
                                className="font-mono text-sm flex-1"
                                placeholder="https://api.example.com/v1..."
                            />
                            <Button onClick={handleSend} disabled={loading} className="w-[100px]">
                                {loading ? <Loader2 className="h-4 w-4 animate-spin" /> : <Send className="h-4 w-4 mr-2" />}
                                Send
                            </Button>
                        </div>
                    </div>

                    <Tabs defaultValue="body" className="flex-1 flex flex-col min-h-0">
                        <div className="px-4 border-b">
                            <TabsList className="h-9 w-full justify-start bg-transparent p-0">
                                <TabsTrigger value="body" className="data-[state=active]:bg-transparent data-[state=active]:shadow-none data-[state=active]:border-b-2 data-[state=active]:border-primary rounded-none h-9 px-4">Body</TabsTrigger>
                                <TabsTrigger value="headers" className="data-[state=active]:bg-transparent data-[state=active]:shadow-none data-[state=active]:border-b-2 data-[state=active]:border-primary rounded-none h-9 px-4">Headers</TabsTrigger>
                            </TabsList>
                        </div>
                        <TabsContent value="body" className="flex-1 p-4 m-0 min-h-0 relative">
                            <Textarea
                                className="font-mono text-xs h-full resize-none border-0 focus-visible:ring-0 p-0 leading-relaxed bg-transparent"
                                value={body}
                                onChange={(e: React.ChangeEvent<HTMLTextAreaElement>) => setBody(e.target.value)}
                                spellCheck={false}
                            />
                        </TabsContent>
                        <TabsContent value="headers" className="flex-1 p-4 m-0 min-h-0">
                            <Textarea
                                className="font-mono text-xs h-full resize-none border-0 focus-visible:ring-0 p-0 leading-relaxed bg-transparent"
                                value={headers}
                                onChange={(e: React.ChangeEvent<HTMLTextAreaElement>) => setHeaders(e.target.value)}
                                spellCheck={false}
                                placeholder="Key: Value"
                            />
                        </TabsContent>
                    </Tabs>
                </CardContent>
            </Card>

            {/* Response Panel */}
            <Card className="flex flex-col h-full border-border/60 shadow-sm overflow-hidden bg-muted/10">
                <CardHeader className="py-3 px-4 border-b bg-muted/20 flex flex-row items-center justify-between h-[57px]">
                    <div className="flex items-center gap-2">
                        <span className="font-semibold text-sm">Response</span>
                        {responseStatus && (
                            <Badge variant={responseStatus < 300 ? "success" : "destructive"} className="ml-2 text-[10px] px-1.5 h-5">
                                {responseStatus}
                            </Badge>
                        )}
                    </div>
                </CardHeader>
                <CardContent className="flex-1 p-0 overflow-auto relative">
                    {loading ? (
                        <div className="absolute inset-0 flex items-center justify-center bg-background/50 backdrop-blur-[1px]">
                            <div className="flex flex-col items-center gap-2">
                                <Loader2 className="h-8 w-8 animate-spin text-primary" />
                                <span className="text-xs text-muted-foreground">Sending request...</span>
                            </div>
                        </div>
                    ) : response ? (
                        <pre className="p-4 text-xs font-mono whitespace-pre-wrap break-all text-foreground/90">
                            {response}
                        </pre>
                    ) : (
                        <div className="h-full flex flex-col items-center justify-center text-muted-foreground/40">
                            <Send className="h-12 w-12 mb-4 opacity-20" />
                            <p className="text-sm">Send a request to see the response here</p>
                        </div>
                    )}
                </CardContent>
            </Card>
        </div>
    );
}
