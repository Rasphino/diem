// A test case which reproduces a performance/non-termination problem. See the spec of fun create for details.

module 0x42::Test {
    use Std::BCS;
    use Std::Signer;
    use Std::Vector;

    spec module {
        fun eq_append<Element>(v: vector<Element>, v1: vector<Element>, v2: vector<Element>): bool {
            len(v) == len(v1) + len(v2) &&
            v[0..len(v1)] == v1 &&
            v[len(v1)..len(v)] == v2
        }
    }

    struct EventHandle<phantom T: copy + drop + store> has copy, drop, store {
        // Total number of events emitted to this event stream.
        counter: u64,
        // A globally unique ID for this event stream.
        guid: vector<u8>,
    }

    struct Event1 has copy, drop, store {}
    struct Event2 has copy, drop, store {}

    struct T has key {
        received_events: EventHandle<Event1>,
        sent_events: EventHandle<Event2>,
    }

    struct EventHandleGenerator has copy, drop, store {
        // A monotonically increasing counter
        counter: u64,
    }

    fun fresh_guid(counter: &mut EventHandleGenerator, sender: address): vector<u8> {
        let sender_bytes = BCS::to_bytes(&sender);
        let count_bytes = BCS::to_bytes(&counter.counter);
        counter.counter = counter.counter + 1;

        Vector::append(&mut count_bytes, sender_bytes);

        count_bytes
    }
    spec fresh_guid {
        aborts_if counter.counter + 1 > max_u64();
        ensures eq_append(result, BCS::serialize(old(counter.counter)), BCS::serialize(sender));
    }

    fun new_event_handle_impl<T: copy + drop + store>(counter: &mut EventHandleGenerator, sender: address): EventHandle<T> {
        EventHandle<T> {counter: 0, guid: fresh_guid(counter, sender)}
    }
    spec new_event_handle_impl {
        aborts_if counter.counter + 1 > max_u64();
        ensures eq_append(result.guid, BCS::serialize(old(counter.counter)), BCS::serialize(sender));
        ensures result.counter == 0;
    }

    public fun create(sender: &signer, fresh_address: address, auth_key_prefix: vector<u8>) : vector<u8> {
        let generator = EventHandleGenerator{counter: 0};
        let authentication_key = auth_key_prefix;
        Vector::append(&mut authentication_key, BCS::to_bytes(&fresh_address));
        assert(Vector::length(&authentication_key) == 32, 12);


        move_to<T>(sender, T{
            received_events: new_event_handle_impl<Event1>(&mut generator, fresh_address),
            sent_events: new_event_handle_impl<Event2>(&mut generator, fresh_address)
        });

        authentication_key
    }
    spec create {
        // To reproduce, put this to true.
        pragma verify=false;

        // The next two aborts_if and ensures are correct. However, if they are removed, verification terminates
        // with the expected result.
        aborts_if len(BCS::serialize(fresh_address)) + len(auth_key_prefix) != 32;
        aborts_if exists<T>(Signer::address_of(sender));
        ensures eq_append(result, auth_key_prefix, BCS::serialize(fresh_address));

        // These two ensures are wrong and should produce an error. Instead, the solver hangs without bounding
        // serialization result size. To reproduce, set --serialize-bound=0 to remove any bound.
        ensures eq_append(global<T>(Signer::address_of(sender)).received_events.guid, BCS::serialize(2), BCS::serialize(fresh_address));
        ensures eq_append(global<T>(Signer::address_of(sender)).sent_events.guid, BCS::serialize(3), BCS::serialize(fresh_address));

        // Correct version of the above ensures:
        //ensures eq_append(global<T>(Signer::address_of(sender)).received_events.guid, BCS::serialize(0), BCS::serialize(fresh_address));
        //ensures eq_append(global<T>(Signer::address_of(sender)).sent_events.guid, BCS::serialize(1), BCS::serialize(fresh_address));
    }
}
