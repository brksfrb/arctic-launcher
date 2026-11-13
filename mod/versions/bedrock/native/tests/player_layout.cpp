// Headless compiler ABI probe, not a Minecraft integration test.
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <string>
#include <unordered_map>
struct Uuid { std::uint64_t a,b; bool operator==(Uuid const&) const=default; };
struct Hash { std::size_t operator()(Uuid const& id) const { return id.a^(522133279*id.b); } };
struct SkinRef { void* implementation; void* owner; };
struct Entry {
    char prefix[0x18]; std::string name;
    char middle[0x80-0x18-sizeof(std::string)]; SkinRef skin;
    char suffix[0xA8-0x80-sizeof(SkinRef)];
};
int main() {
    static_assert(sizeof(std::string)==32 && sizeof(Entry)==0xA8);
    static_assert(offsetof(Entry,name)==0x18 && offsetof(Entry,skin)==0x80);
    std::unordered_map<Uuid,Entry,Hash> players;
    players[{1,2}].name="ArcticTest";
    void* head;std::size_t count;
    std::memcpy(&head,reinterpret_cast<char*>(&players)+8,8);
    std::memcpy(&count,reinterpret_cast<char*>(&players)+16,8);
    void* node;std::memcpy(&node,head,8);
    if(count!=1 || reinterpret_cast<char*>(&*players.begin())-reinterpret_cast<char*>(node)!=16) return 1;
    if(reinterpret_cast<char*>(&players.begin()->second)-reinterpret_cast<char*>(node)!=32) return 2;
    std::printf("{\"mapSize\":%zu,\"listHead\":8,\"listCount\":16,\"nodeKey\":16,\"nodeEntry\":32,\"name\":24,\"skin\":128}\n",sizeof(players));
}
